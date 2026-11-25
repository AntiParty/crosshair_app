//! The Finals Crosshair mark: a red badge with a white crosshair. The same shape is used on the website.
//! Drawn in code so the app needs no image files: once into pixels for the window icon, once with the painter
//! for the title bar.

use eframe::egui::{self, Color32, Pos2, Rect, Ui, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(0xff, 0x4d, 0x5e);

/// The four bars and the dot, in a 28 x 28 design space (the badge is the circle of radius 14 around 14,14).
const BARS: [(f32, f32, f32, f32); 4] = [
    (12.6, 5.2, 15.4, 10.2),  // top
    (12.6, 17.8, 15.4, 22.8), // bottom
    (5.2, 12.6, 10.2, 15.4),  // left
    (17.8, 12.6, 22.8, 15.4), // right
];
const DOT_R: f32 = 1.7;
const BAR_ROUND: f32 = 0.9;

fn in_bar(x: f32, y: f32, (x0, y0, x1, y1): (f32, f32, f32, f32)) -> bool {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let (hw, hh) = ((x1 - x0) / 2.0, (y1 - y0) / 2.0);
    let dx = ((x - cx).abs() - hw + BAR_ROUND).max(0.0);
    let dy = ((y - cy).abs() - hh + BAR_ROUND).max(0.0);
    dx * dx + dy * dy <= BAR_ROUND * BAR_ROUND
}

fn in_white(x: f32, y: f32) -> bool {
    BARS.iter().any(|b| in_bar(x, y, *b))
        || (x - 14.0).powi(2) + (y - 14.0).powi(2) <= DOT_R * DOT_R
}

/// RGBA pixels (straight alpha) for a `size` x `size` window icon, 4x4 supersampled.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    let n = 4;
    for py in 0..size {
        for px in 0..size {
            let (mut a, mut w) = (0.0f32, 0.0f32);
            for sy in 0..n {
                for sx in 0..n {
                    let x = (px as f32 + (sx as f32 + 0.5) / n as f32) / size as f32 * 28.0;
                    let y = (py as f32 + (sy as f32 + 0.5) / n as f32) / size as f32 * 28.0;
                    if (x - 14.0).powi(2) + (y - 14.0).powi(2) <= 14.0 * 14.0 {
                        a += 1.0;
                        if in_white(x, y) {
                            w += 1.0;
                        }
                    }
                }
            }
            let total = (n * n) as f32;
            let white = if a > 0.0 { w / a } else { 0.0 };
            let mix = |accent: u8| (accent as f32 * (1.0 - white) + 255.0 * white).round() as u8;
            out.extend_from_slice(&[
                mix(ACCENT.r()),
                mix(ACCENT.g()),
                mix(ACCENT.b()),
                (a / total * 255.0).round() as u8,
            ]);
        }
    }
    out
}

/// Paints the mark `size` points wide and returns the space it used.
pub fn paint(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let k = size / 28.0;
    let at = |x: f32, y: f32| Pos2::new(rect.min.x + x * k, rect.min.y + y * k);
    let p = ui.painter();
    p.circle_filled(rect.center(), size / 2.0, ACCENT);
    for (x0, y0, x1, y1) in BARS {
        p.rect_filled(
            Rect::from_min_max(at(x0, y0), at(x1, y1)),
            BAR_ROUND * k,
            Color32::WHITE,
        );
    }
    p.circle_filled(at(14.0, 14.0), DOT_R * k, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_the_right_size_and_shape() {
        let px = icon_rgba(64);
        assert_eq!(px.len(), 64 * 64 * 4);
        let at = |x: usize, y: usize| &px[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4];
        assert_eq!(at(0, 0)[3], 0, "the corner is outside the badge");
        assert_eq!(at(32, 32)[3], 255, "the middle is solid");
        assert_eq!(
            &at(32, 32)[..3],
            &[255, 255, 255],
            "the centre dot is white"
        );
        let on_badge = at(4, 32); // between the edge and the left bar
        assert_eq!(on_badge[3], 255);
        assert_eq!(
            &on_badge[..3],
            &[0xff, 0x4d, 0x5e],
            "the badge is the accent colour"
        );
    }
}
