use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText, Ui};

use crate::guard;
use crate::model::{self, CENTER_SHAPES, Crosshair, MELEE_LOOKS, RETICLE_SHAPES, SIGHT_SHAPES};
use crate::preview::{self, Params};
use crate::site;
use crate::store::{self, Backup};

const ACCENT: Color32 = Color32::from_rgb(0xFF, 0x5A, 0x36);
const OK: Color32 = Color32::from_rgb(0x3D, 0xBE, 0x7A);
const WARN: Color32 = Color32::from_rgb(0xE3, 0xA9, 0x2B);
const POLL: Duration = Duration::from_secs(2);
const ARM_FOR: Duration = Duration::from_secs(5);

#[derive(PartialEq, Clone, Copy)]
enum View {
    Crosshair,
    Scope,
}

enum Tone {
    Info,
    Good,
    Bad,
}

/// Where a background download leaves its answer.
type FetchSlot = Arc<Mutex<Option<Result<Crosshair, String>>>>;

pub struct App {
    cur: Crosshair,
    presets: Vec<Crosshair>,
    backups: Vec<Backup>,
    save_path: PathBuf,
    game: Option<String>,
    game_shared: Arc<Mutex<Option<String>>>,
    view: View,
    backdrop: usize,
    zoom: f32,
    spread: f32,
    animate: bool,
    melee: bool,
    status: (String, Tone),
    armed: Option<Instant>,
    import_text: String,
    show_import: bool,
    /// An import-by-link that is downloading on its own thread; the answer lands here.
    fetch: Option<FetchSlot>,
}

pub fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = Color32::from_rgb(0x17, 0x19, 0x1e);
    v.window_fill = Color32::from_rgb(0x1d, 0x20, 0x26);
    v.extreme_bg_color = Color32::from_rgb(0x11, 0x13, 0x17);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.75);
    v.hyperlink_color = ACCENT;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 7.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.slider_width = 150.0;
        for (_, f) in s.text_styles.iter_mut() {
            f.size *= 1.08;
        }
    });
}

/// Polls the process list on its own thread; requests a repaint only when the answer changes.
fn watch_game(ctx: &egui::Context) -> Arc<Mutex<Option<String>>> {
    let shared = Arc::new(Mutex::new(guard::running_game()));
    let (s, ctx) = (shared.clone(), ctx.clone());
    std::thread::spawn(move || {
        let mut last = s.lock().ok().and_then(|g| g.clone());
        loop {
            std::thread::sleep(POLL);
            let now = guard::running_game();
            if now != last {
                if let Ok(mut g) = s.lock() {
                    *g = now.clone();
                }
                last = now;
                ctx.request_repaint();
            }
        }
    });
    shared
}

impl App {
    pub fn new(ctx: &egui::Context) -> App {
        let save_path = store::save_path();
        let game_shared = watch_game(ctx);
        let (cur, status) = match store::read_current(&save_path) {
            Ok(c) => (
                c,
                (
                    "Loaded your current in-game crosshair.".to_string(),
                    Tone::Info,
                ),
            ),
            Err(e) => (Crosshair::defaults(), (e, Tone::Bad)),
        };
        App {
            cur,
            presets: store::list_presets(),
            backups: store::list_backups(),
            save_path,
            game: game_shared.lock().ok().and_then(|g| g.clone()),
            game_shared,
            view: if std::env::var("FC_VIEW").as_deref() == Ok("scope") {
                View::Scope
            } else {
                View::Crosshair
            },
            backdrop: std::env::var("FC_BACKDROP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
                .min(preview::BACKDROPS.len() - 1),
            zoom: 3.0,
            spread: 8.0,
            animate: false,
            melee: std::env::var("FC_MELEE").is_ok(),
            status,
            armed: None,
            import_text: String::new(),
            show_import: false,
            fetch: None,
        }
    }

    /// Applies the answer of a finished download, if there is one.
    fn poll_fetch(&mut self) {
        let Some(slot) = &self.fetch else { return };
        let done = slot.lock().ok().and_then(|mut r| r.take());
        if let Some(result) = done {
            self.fetch = None;
            match result {
                Ok(c) => {
                    self.say(
                        Tone::Good,
                        format!("Imported '{}'. Save it to keep it.", c.name),
                    );
                    self.cur = c;
                    self.import_text.clear();
                    self.show_import = false;
                }
                Err(e) => self.say(Tone::Bad, e),
            }
        }
    }

    /// Starts downloading a crosshair from the site without freezing the window.
    fn start_fetch(&mut self, ctx: &egui::Context, id: String) {
        let slot = Arc::new(Mutex::new(None));
        self.fetch = Some(slot.clone());
        self.say(Tone::Info, "Downloading from the site…");
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = site::fetch(&id);
            if let Ok(mut r) = slot.lock() {
                *r = Some(result);
            }
            ctx.request_repaint();
        });
    }

    fn say(&mut self, tone: Tone, msg: impl Into<String>) {
        self.status = (msg.into(), tone);
    }

    /// The background watcher wakes the UI only when the game starts or stops, so an idle window costs nothing.
    fn sync_game(&mut self) {
        if let Ok(g) = self.game_shared.lock() {
            self.game = g.clone();
        }
    }

    // ── actions ─────────────────────────────────────────────────────────────

    fn load_from_game(&mut self) {
        match store::read_current(&self.save_path) {
            Ok(c) => {
                self.cur = c;
                self.say(Tone::Info, "Loaded your current in-game crosshair.");
            }
            Err(e) => self.say(Tone::Bad, e),
        }
    }

    fn save_preset(&mut self) {
        let name = self.cur.name.trim().to_string();
        if name.is_empty() || name == "Current in game" {
            self.say(Tone::Bad, "Give the preset a name first.");
            return;
        }
        self.cur.name = name.clone();
        match store::save_preset(&self.cur) {
            Ok(()) => {
                self.presets = store::list_presets();
                self.say(Tone::Good, format!("Saved '{name}'."));
            }
            Err(e) => self.say(Tone::Bad, format!("Couldn't save: {e}")),
        }
    }

    fn apply(&mut self) {
        match store::apply(&self.save_path, &self.cur) {
            Ok(a) => {
                self.backups = store::list_backups();
                let n = a.changed;
                let bk = a
                    .backup
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.say(
                    Tone::Good,
                    format!(
                        "Applied: {n} setting{} changed. Backup: {bk}. Start the game to see it.",
                        if n == 1 { "" } else { "s" }
                    ),
                );
            }
            Err(e) => self.say(Tone::Bad, e),
        }
    }

    fn restore(&mut self, path: PathBuf) {
        match store::restore(&self.save_path, &path) {
            Ok(()) => self.say(Tone::Good, "Restored that backup."),
            Err(e) => self.say(Tone::Bad, e),
        }
    }

    // ── panels ──────────────────────────────────────────────────────────────

    fn top_bar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            crate::logo::paint(ui, 24.0);
            ui.label(RichText::new("Finals Crosshair").size(19.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (text, col) = match &self.game {
                    None => ("Game closed".to_string(), OK),
                    Some(n) => (format!("Game running ({n})"), WARN),
                };
                ui.label(RichText::new(text).color(col).strong());
                let (dot, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().circle_filled(dot.center(), 5.0, col);
            });
        });
    }

    fn presets_panel(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("Presets").strong().size(15.0));
        ui.horizontal(|ui| {
            if ui
                .button("From game")
                .on_hover_text("Read the crosshair you currently have in the game")
                .clicked()
            {
                self.load_from_game();
            }
            if ui.button("Defaults").clicked() {
                let name = self.cur.name.clone();
                self.cur = Crosshair::defaults();
                self.cur.name = name;
            }
        });
        ui.separator();
        let mut pick = None;
        let mut delete = None;
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 150.0)
            .show(ui, |ui| {
                if self.presets.is_empty() {
                    ui.label(
                        RichText::new(
                            "No saved presets yet. Name the current crosshair below and save it.",
                        )
                        .weak(),
                    );
                }
                for (i, p) in self.presets.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let on = self.cur.name == p.name;
                        if ui.selectable_label(on, &p.name).clicked() {
                            pick = Some(i);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .small_button("×")
                                .on_hover_text("Delete this preset")
                                .clicked()
                            {
                                delete = Some(i);
                            }
                        });
                    });
                }
            });
        if let Some(i) = pick {
            self.cur = self.presets[i].clone();
        }
        if let Some(i) = delete {
            store::delete_preset(&self.presets[i].name);
            self.presets = store::list_presets();
        }
        ui.separator();
        ui.label("Name");
        ui.add(egui::TextEdit::singleline(&mut self.cur.name).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            if ui.button("Save preset").clicked() {
                self.save_preset();
            }
            if ui
                .button("Copy code")
                .on_hover_text("Copy this crosshair as text to share")
                .clicked()
            {
                ui.ctx().copy_text(self.cur.to_share_code());
                self.say(Tone::Info, "Copied. Paste it to a friend or into Import.");
            }
        });
        if ui
            .button(if self.show_import {
                "Hide import"
            } else {
                "Import code…"
            })
            .clicked()
        {
            self.show_import = !self.show_import;
        }
        if self.show_import {
            ui.add(
                egui::TextEdit::multiline(&mut self.import_text)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY)
                    .hint_text("Paste a crosshair code, or a link from the site"),
            );
            let idle = self.fetch.is_none();
            if ui.add_enabled(idle, egui::Button::new("Import")).clicked() {
                if let Some(id) = site::parse_target(&self.import_text) {
                    self.start_fetch(ui.ctx(), id);
                } else {
                    match Crosshair::from_share_code(&self.import_text) {
                        Ok(c) => {
                            self.say(
                                Tone::Good,
                                format!("Imported '{}'. Save it to keep it.", c.name),
                            );
                            self.cur = c;
                            self.import_text.clear();
                            self.show_import = false;
                        }
                        Err(e) => self.say(Tone::Bad, e),
                    }
                }
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui
                .button("Browse online")
                .on_hover_text("Open the crosshair site in your browser")
                .clicked()
            {
                site::open(&site::browse_url());
            }
            if ui
                .button("Publish online")
                .on_hover_text("Copy this crosshair's code and open the site's Publish page")
                .clicked()
            {
                ui.ctx().copy_text(self.cur.to_share_code());
                site::open(&site::publish_url());
                self.say(
                    Tone::Info,
                    "Copied your code. Paste it on the Publish page that just opened.",
                );
            }
        });
    }

    fn editor_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let c = &mut self.cur;
                section(ui, "Center", true, |ui| {
                    piece(
                        ui,
                        c,
                        "GameplayOption.Crosshair.Center",
                        CENTER_SHAPES,
                        true,
                        false,
                    );
                });
                section(ui, "Moving reticle (spreads when you shoot)", true, |ui| {
                    piece(
                        ui,
                        c,
                        "GameplayOption.Crosshair.DispersionReticle",
                        RETICLE_SHAPES,
                        false,
                        true,
                    );
                });
                section(ui, "Fixed reticle", false, |ui| {
                    piece(
                        ui,
                        c,
                        "GameplayOption.Crosshair.StaticReticle",
                        RETICLE_SHAPES,
                        false,
                        true,
                    );
                    fslider(
                        ui,
                        c,
                        "GameplayOption.Crosshair.StaticReticle.InnerOffset",
                        "Gap from center",
                    );
                });
                section(ui, "Melee", false, |ui| {
                    choice(
                        ui,
                        c,
                        "GameplayOption.Crosshair.MeleeLook",
                        "Melee crosshair",
                        MELEE_LOOKS,
                    );
                    piece(
                        ui,
                        c,
                        "GameplayOption.Crosshair.MeleeCenter",
                        CENTER_SHAPES,
                        true,
                        false,
                    );
                    flag(
                        ui,
                        c,
                        "GameplayOption.Crosshair.MeleeInWorldCrosshair",
                        "Show in the world",
                    );
                });
                section(ui, "Scope sight", false, |ui| {
                    flag(
                        ui,
                        c,
                        "GameplayOption.Sights.Enabled",
                        "Use my sight settings",
                    );
                    choice(
                        ui,
                        c,
                        "GameplayOption.Sights.ReticleShape",
                        "Shape",
                        SIGHT_SHAPES,
                    );
                    fslider(ui, c, "GameplayOption.Sights.ReticleSize", "Size");
                    rgb(ui, c, "GameplayOption.Sights.Color", "Color");
                    let shape = c.choice("GameplayOption.Sights.ReticleShape");
                    for (k, label) in [
                        ("DotSize", "Dot size"),
                        ("Thickness", "Thickness"),
                        ("Length", "Length"),
                        ("Gap", "Gap"),
                        ("Width", "Width"),
                    ] {
                        let key = format!("GameplayOption.Sights.ReticleShape.{shape}.{k}");
                        if model::float_spec(&key).is_some() {
                            fslider(ui, c, &key, label);
                        }
                    }
                });
                section(ui, "Other", false, |ui| {
                    flag(
                        ui,
                        c,
                        "GameplayOption.Crosshair.InWorldCrosshair",
                        "Show the crosshair in the world",
                    );
                });
            });
    }

    fn bottom_bar(&mut self, ui: &mut Ui) {
        if let Some(t) = self.armed {
            if t.elapsed() > ARM_FOR {
                self.armed = None;
            } else {
                ui.ctx().request_repaint_after(ARM_FOR - t.elapsed());
            }
        }
        ui.horizontal(|ui| {
            let (msg, tone) = &self.status;
            let col = match tone {
                Tone::Info => ui.visuals().weak_text_color(),
                Tone::Good => OK,
                Tone::Bad => Color32::from_rgb(0xF0, 0x6A, 0x5A),
            };
            ui.label(RichText::new(msg).color(col));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can = self.game.is_none() && self.save_path.exists();
                let armed = self.armed.is_some();
                let label = if armed { "Click again to confirm" } else { "Apply to game" };
                let btn = egui::Button::new(RichText::new(label).strong().color(Color32::WHITE)).fill(if armed { WARN.gamma_multiply(0.8) } else { ACCENT });
                let r = ui.add_enabled(can, btn);
                let r = if self.game.is_some() {
                    r.on_disabled_hover_text("Close THE FINALS first. The game would overwrite the change.")
                } else {
                    r.on_hover_text("Writes this crosshair into the game's settings file. A backup is made first.")
                };
                if r.clicked() {
                    if armed {
                        self.armed = None;
                        self.apply();
                    } else {
                        self.armed = Some(Instant::now());
                    }
                }
                if !self.backups.is_empty() {
                    ui.menu_button("Restore backup", |ui| {
                        let mut chosen = None;
                        for b in self.backups.iter().take(12) {
                            if ui.add_enabled(self.game.is_none(), egui::Button::new(&b.label)).clicked() {
                                chosen = Some(b.path.clone());
                                ui.close();
                            }
                        }
                        if let Some(p) = chosen {
                            self.restore(p);
                        }
                    });
                }
            });
        });
    }

    fn preview_panel(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().slider_width = 70.0;
            ui.selectable_value(&mut self.view, View::Crosshair, "Crosshair");
            ui.selectable_value(&mut self.view, View::Scope, "Scope");
            ui.separator();
            ui.label("Backdrop");
            egui::ComboBox::from_id_salt("backdrop")
                .selected_text(preview::BACKDROPS[self.backdrop].0)
                .show_ui(ui, |ui| {
                    for (i, (n, _)) in preview::BACKDROPS.iter().enumerate() {
                        ui.selectable_value(&mut self.backdrop, i, *n);
                    }
                });
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.zoom, 1.0..=8.0).step_by(0.5));
            if self.view == View::Crosshair {
                ui.checkbox(&mut self.melee, "Melee");
                if !self.melee {
                    ui.checkbox(&mut self.animate, "Shoot")
                        .on_hover_text("Animate the spread of the moving reticle");
                    ui.add_enabled(
                        !self.animate,
                        egui::Slider::new(&mut self.spread, 0.0..=30.0).text("Spread"),
                    );
                }
            }
        });

        let (rect, _) = ui.allocate_exact_size(
            ui.available_size() - egui::vec2(0.0, 22.0),
            egui::Sense::hover(),
        );
        let p = ui.painter_at(rect);
        preview::draw_backdrop(&p, rect, self.backdrop);
        let at = rect.center();
        if self.view == View::Crosshair {
            let c = &self.cur;
            let on = |b: &str| c.flag(&format!("GameplayOption.Crosshair.{b}.Enabled"));
            let melee_shows = if c.choice("GameplayOption.Crosshair.MeleeLook") == "Same" {
                on("Center")
            } else {
                on("MeleeCenter")
            };
            let nothing = if self.melee {
                !melee_shows
            } else {
                !(on("Center") || on("StaticReticle") || on("DispersionReticle"))
            };
            if nothing {
                let msg = if self.melee {
                    "Melee crosshair is off in these settings"
                } else {
                    "No crosshair parts are switched on"
                };
                p.text(
                    at,
                    egui::Align2::CENTER_CENTER,
                    msg,
                    egui::FontId::proportional(15.0),
                    Color32::from_rgba_unmultiplied(120, 120, 120, 200),
                );
            }
        }
        match self.view {
            View::Crosshair => {
                let spread = if self.animate && !self.melee {
                    ui.ctx().request_repaint_after(Duration::from_millis(33));
                    let t = ui.input(|i| i.time) as f32;
                    2.0 + 22.0 * (0.5 + 0.5 * (t * 4.0).sin()).powf(2.0)
                } else {
                    self.spread
                };
                preview::draw_crosshair(
                    &p,
                    at,
                    &self.cur,
                    &Params {
                        scale: self.zoom,
                        spread,
                        melee: self.melee,
                    },
                );
            }
            View::Scope => {
                let radius = rect.height().min(rect.width()) * 0.42;
                preview::lens(&p, at, radius);
                preview::draw_sight(&p, at, &self.cur, self.zoom * 2.0);
            }
        }
        ui.label(RichText::new("1× is roughly real size at 1080p. Exact in-game sizing isn't published, so treat the preview as a close guide.").weak().small());
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.sync_game();
        self.poll_fetch();
        egui::Panel::top("top").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("bottom").show(ui, |ui| self.bottom_bar(ui));
        egui::Panel::left("presets")
            .exact_size(224.0)
            .resizable(false)
            .show(ui, |ui| self.presets_panel(ui));
        egui::Panel::right("editor")
            .exact_size(336.0)
            .resizable(false)
            .show(ui, |ui| self.editor_panel(ui));
        egui::CentralPanel::default().show(ui, |ui| self.preview_panel(ui));
    }
}

// ── small editor widgets ────────────────────────────────────────────────────

fn section(ui: &mut Ui, title: &str, open: bool, body: impl FnOnce(&mut Ui)) {
    egui::CollapsingHeader::new(RichText::new(title).strong())
        .default_open(open)
        .show(ui, body);
}

fn fslider(ui: &mut Ui, c: &mut Crosshair, key: &str, label: &str) {
    let Some((_, lo, hi)) = model::float_spec(key) else {
        return;
    };
    let mut v = c.float(key);
    let step = if hi - lo > 20.0 { 1.0 } else { 0.05 };
    if ui
        .add(egui::Slider::new(&mut v, lo..=hi).step_by(step).text(label))
        .changed()
    {
        c.set_float(key, v);
    }
}

fn flag(ui: &mut Ui, c: &mut Crosshair, key: &str, label: &str) {
    let mut b = c.flag(key);
    if ui.checkbox(&mut b, label).changed() {
        c.set_flag(key, b);
    }
}

fn rgb(ui: &mut Ui, c: &mut Crosshair, prefix: &str, label: &str) {
    ui.horizontal(|ui| {
        let mut v = c.rgb(prefix);
        if egui::color_picker::color_edit_button_srgb(ui, &mut v).changed() {
            c.set_rgb(prefix, v);
        }
        ui.label(label);
    });
}

fn choice(ui: &mut Ui, c: &mut Crosshair, key: &str, label: &str, options: &[&str]) {
    let cur = c.choice(key);
    ui.horizontal(|ui| {
        ui.label(label);
        egui::ComboBox::from_id_salt(key)
            .selected_text(cur.clone())
            .show_ui(ui, |ui| {
                for o in options {
                    if ui.selectable_label(cur == *o, *o).clicked() {
                        c.set_choice(key, o);
                    }
                }
            });
    });
}

/// One crosshair element: on/off, shape, size, color, opacity and its outline.
fn piece(ui: &mut Ui, c: &mut Crosshair, base: &str, shapes: &[&str], radius: bool, bar: bool) {
    let k = |s: &str| format!("{base}.{s}");
    flag(ui, c, &k("Enabled"), "On");
    choice(ui, c, &k("Shape"), "Shape", shapes);
    if radius {
        fslider(ui, c, &k("Radius"), "Radius");
    }
    if bar {
        fslider(ui, c, &k("Length"), "Length");
        fslider(ui, c, &k("Thickness"), "Thickness");
    }
    rgb(ui, c, &k("Color"), "Color");
    fslider(ui, c, &k("Opacity"), "Opacity");
    ui.collapsing("Outline", |ui| {
        rgb(ui, c, &k("Outline.Color"), "Color");
        fslider(ui, c, &k("Outline.Opacity"), "Opacity");
        fslider(ui, c, &k("Outline.Thickness"), "Thickness");
    });
}
