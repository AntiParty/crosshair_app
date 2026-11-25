//! Draws the crosshair from the game's own setting values. Sizes are the game's units times `scale`
//! (1.0 ~ real size at 1080p; the in-game scaling isn't published, so treat sizes as approximate).

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use crate::model::Crosshair;

pub struct Params {
    pub scale: f32,
    /// How far the moving reticle has spread, in the same units as lengths.
    pub spread: f32,
    pub melee: bool,
}

fn color(rgb: [u8; 3], opacity_percent: f32) -> Color32 {
    let a = (opacity_percent.clamp(0.0, 100.0) * 2.55).round() as u8;
    Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], a)
}

fn rect(cx: f32, cy: f32, x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect::from_min_max(Pos2::new(cx + x0, cy + y0), Pos2::new(cx + x1, cy + y1))
}

/// A bar with an outline: the outline is the same shape grown by its thickness, drawn first.
fn bar(p: &Painter, r: Rect, fill: Color32, outline: Color32, outline_px: f32) {
    if outline_px > 0.0 {
        p.rect_filled(r.expand(outline_px), 0.0, outline);
    }
    p.rect_filled(r, 0.0, fill);
}

fn ring(
    p: &Painter,
    c: Pos2,
    radius: f32,
    width: f32,
    fill: Color32,
    outline: Color32,
    outline_px: f32,
) {
    if outline_px > 0.0 {
        p.circle_stroke(c, radius, Stroke::new(width + outline_px * 2.0, outline));
    }
    p.circle_stroke(c, radius, Stroke::new(width, fill));
}

fn disc(p: &Painter, c: Pos2, radius: f32, fill: Color32, outline: Color32, outline_px: f32) {
    if outline_px > 0.0 {
        p.circle_filled(c, radius + outline_px, outline);
    }
    p.circle_filled(c, radius, fill);
}

fn center_piece(p: &Painter, at: Pos2, c: &Crosshair, base: &str, s: f32) {
    if !c.flag(&format!("{base}.Enabled")) {
        return;
    }
    let r = c.float(&format!("{base}.Radius")) * s;
    let fill = color(
        c.rgb(&format!("{base}.Color")),
        c.float(&format!("{base}.Opacity")),
    );
    let oc = color(
        c.rgb(&format!("{base}.Outline.Color")),
        c.float(&format!("{base}.Outline.Opacity")),
    );
    let ot = c.float(&format!("{base}.Outline.Thickness")) * s;
    if c.choice(&format!("{base}.Shape")) == "Circle" {
        ring(p, at, r, (s).max(1.0), fill, oc, ot);
    } else {
        disc(p, at, r, fill, oc, ot);
    }
}

fn reticle(p: &Painter, at: Pos2, c: &Crosshair, base: &str, inner_offset: f32, s: f32) {
    if !c.flag(&format!("{base}.Enabled")) {
        return;
    }
    let len = c.float(&format!("{base}.Length")) * s;
    let th = c.float(&format!("{base}.Thickness")) * s;
    let off = inner_offset * s;
    let fill = color(
        c.rgb(&format!("{base}.Color")),
        c.float(&format!("{base}.Opacity")),
    );
    let oc = color(
        c.rgb(&format!("{base}.Outline.Color")),
        c.float(&format!("{base}.Outline.Opacity")),
    );
    let ot = c.float(&format!("{base}.Outline.Thickness")) * s;
    let (cx, cy) = (at.x, at.y);
    match c.choice(&format!("{base}.Shape")).as_str() {
        "Circle" => ring(p, at, off + len * 0.5, th, fill, oc, ot),
        shape => {
            let h = th * 0.5;
            let mut arms = vec![
                rect(cx, cy, -off - len, -h, -off, h), // left
                rect(cx, cy, off, -h, off + len, h),   // right
                rect(cx, cy, -h, off, h, off + len),   // down
            ];
            if shape != "TShape" {
                arms.push(rect(cx, cy, -h, -off - len, h, -off)); // up
            }
            for r in arms {
                bar(p, r, fill, oc, ot);
            }
        }
    }
}

/// The on-screen crosshair (or the melee one).
pub fn draw_crosshair(p: &Painter, at: Pos2, c: &Crosshair, prm: &Params) {
    let s = prm.scale;
    if prm.melee {
        if c.choice("GameplayOption.Crosshair.MeleeLook") == "Same" {
            center_piece(p, at, c, "GameplayOption.Crosshair.Center", s);
        } else {
            center_piece(p, at, c, "GameplayOption.Crosshair.MeleeCenter", s);
        }
        return;
    }
    reticle(
        p,
        at,
        c,
        "GameplayOption.Crosshair.StaticReticle",
        c.float("GameplayOption.Crosshair.StaticReticle.InnerOffset"),
        s,
    );
    reticle(
        p,
        at,
        c,
        "GameplayOption.Crosshair.DispersionReticle",
        prm.spread,
        s,
    );
    center_piece(p, at, c, "GameplayOption.Crosshair.Center", s);
}

/// The reticle drawn inside a scope. Units are the sight editor's own; `s` is pixels per unit.
pub fn draw_sight(p: &Painter, at: Pos2, c: &Crosshair, s: f32) {
    if !c.flag("GameplayOption.Sights.Enabled") {
        return;
    }
    let size = c.float("GameplayOption.Sights.ReticleSize");
    let u = s * size;
    let col = color(c.rgb("GameplayOption.Sights.Color"), 100.0);
    let shape = c.choice("GameplayOption.Sights.ReticleShape");
    let f = |k: &str| c.float(&format!("GameplayOption.Sights.ReticleShape.{shape}.{k}"));
    let line = |a: Pos2, b: Pos2, w: f32| p.line_segment([a, b], Stroke::new(w, col));
    let dot = |r: f32| {
        if r > 0.0 {
            p.circle_filled(at, r, col);
        }
    };
    match shape.as_str() {
        "Dot" => dot(f("DotSize") * u),
        "Circle" => {
            p.circle_stroke(
                at,
                (4.0 + f("Gap")) * u,
                Stroke::new((f("Thickness") * u * 0.5).max(1.0), col),
            );
            dot(f("DotSize") * u * 0.5);
        }
        "Cross" | "TShape" => {
            let (g, l, t) = (
                f("Gap") * u,
                f("Length") * u,
                (f("Thickness") * u * 0.5).max(1.0),
            );
            line(Pos2::new(at.x - g, at.y), Pos2::new(at.x - g - l, at.y), t);
            line(Pos2::new(at.x + g, at.y), Pos2::new(at.x + g + l, at.y), t);
            line(Pos2::new(at.x, at.y + g), Pos2::new(at.x, at.y + g + l), t);
            if shape == "Cross" {
                line(Pos2::new(at.x, at.y - g), Pos2::new(at.x, at.y - g - l), t);
            }
            dot(f("DotSize") * u * 0.5);
        }
        "XShape" => {
            let (g, l, t) = (
                f("Gap") * u,
                f("Length") * u,
                (f("Thickness") * u * 0.5).max(1.0),
            );
            let d = std::f32::consts::FRAC_1_SQRT_2;
            for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                let a = Pos2::new(at.x + sx * g * d, at.y + sy * g * d);
                let b = Pos2::new(at.x + sx * (g + l) * d, at.y + sy * (g + l) * d);
                line(a, b, t);
            }
            dot(f("DotSize") * u * 0.5);
        }
        "Triangle" => {
            let (l, w, t) = (
                f("Length") * u,
                f("Width") * f("Length") * u,
                (f("Thickness") * u * 0.5).max(1.0),
            );
            let pts = vec![
                Pos2::new(at.x, at.y - l * 0.66),
                Pos2::new(at.x - w * 0.5, at.y + l * 0.34),
                Pos2::new(at.x + w * 0.5, at.y + l * 0.34),
            ];
            p.add(Shape::closed_line(pts, Stroke::new(t, col)));
        }
        _ => {}
    }
}

pub const BACKDROPS: &[(&str, [u8; 3])] = &[
    ("Night", [0x15, 0x17, 0x1c]),
    ("Concrete", [0x8c, 0x8f, 0x93]),
    ("Sky", [0x72, 0xa9, 0xd6]),
    ("Brick", [0x9b, 0x4b, 0x3b]),
    ("Grass", [0x52, 0x7d, 0x3d]),
    ("Snow", [0xee, 0xf1, 0xf4]),
    ("Busy", [0x55, 0x58, 0x5e]),
];

/// Fills the preview area. "Busy" adds deterministic blocks of mixed brightness, a hard case for visibility.
pub fn draw_backdrop(p: &Painter, area: Rect, which: usize) {
    let (name, rgb) = BACKDROPS[which.min(BACKDROPS.len() - 1)];
    p.rect_filled(area, 0.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    if name == "Busy" {
        let mut seed: u32 = 0x9E37_79B9;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % 1000) as f32 / 1000.0
        };
        for _ in 0..140 {
            let (x, y, w, h, v) = (
                next(),
                next(),
                0.02 + next() * 0.10,
                0.02 + next() * 0.10,
                next(),
            );
            let r = Rect::from_min_size(
                Pos2::new(
                    area.min.x + area.width() * x,
                    area.min.y + area.height() * y,
                ),
                Vec2::new(area.width() * w, area.height() * h),
            );
            let g = (40.0 + v * 190.0) as u8;
            p.rect_filled(
                r.intersect(area),
                0.0,
                Color32::from_rgb(g, (g as f32 * 0.9) as u8, (g as f32 * 0.8) as u8),
            );
        }
    }
}

pub fn lens(p: &Painter, at: Pos2, radius: f32) {
    p.circle_filled(at, radius, Color32::from_rgba_unmultiplied(0, 0, 0, 70));
    p.circle_stroke(at, radius, Stroke::new(3.0, Color32::from_rgb(20, 20, 24)));
}
