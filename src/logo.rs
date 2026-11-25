//! The Finals Crosshair mark: a dark rounded badge, a white hexagon, a crosshair cut out of it and a red dot.
//! The same shape is used on the website. Drawn in code so the app needs no image files: once into pixels for
//! the window icon, once with the painter for the title bar.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Ui, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(0xff, 0x4d, 0x5e);
const DARK: Color32 = Color32::from_rgb(0x0b, 0x0b, 0x0d);
const WHITE: Color32 = Color32::from_rgb(0xf6, 0xf6, 0xf8);

// Everything below is in a 28 x 28 design space.
const BADGE_ROUND: f32 = 6.5;
const HEX: [(f32, f32); 6] = [
    (24.0, 14.0),
    (19.0, 22.66),
    (9.0, 22.66),
    (4.0, 14.0),
    (9.0, 5.34),
    (19.0, 5.34),
];
/// The crosshair cut out of the hexagon: `(x0, y0, x1, y1)`.
const BARS: [(f32, f32, f32, f32); 4] = [
    (13.0, 7.4, 15.0, 11.4),  // top
    (13.0, 16.6, 15.0, 20.6), // bottom
    (6.6, 13.0, 11.0, 15.0),  // left
    (17.0, 13.0, 21.4, 15.0), // right
];
const DOT_R: f32 = 1.5;

fn in_badge(x: f32, y: f32) -> bool {
    let dx = ((x - 14.0).abs() - 14.0 + BADGE_ROUND).max(0.0);
    let dy = ((y - 14.0).abs() - 14.0 + BADGE_ROUND).max(0.0);
    dx * dx + dy * dy <= BADGE_ROUND * BADGE_ROUND
}

fn in_hex(x: f32, y: f32) -> bool {
    // convex polygon: the point is on the same side of every edge
    let mut sign = 0.0f32;
    for i in 0..6 {
        let (ax, ay) = HEX[i];
        let (bx, by) = HEX[(i + 1) % 6];
        let cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
        if cross != 0.0 {
            if sign == 0.0 {
                sign = cross.signum();
            } else if sign != cross.signum() {
                return false;
            }
        }
    }
    true
}

fn in_bars(x: f32, y: f32) -> bool {
    BARS.iter()
        .any(|&(x0, y0, x1, y1)| x >= x0 && x <= x1 && y >= y0 && y <= y1)
}

/// The colour of one point of the mark, or `None` outside the badge.
fn sample(x: f32, y: f32) -> Option<[f32; 3]> {
    if !in_badge(x, y) {
        return None;
    }
    let c = if (x - 14.0).powi(2) + (y - 14.0).powi(2) <= DOT_R * DOT_R {
        ACCENT
    } else if in_hex(x, y) && !in_bars(x, y) {
        WHITE
    } else {
        DARK
    };
    Some([c.r() as f32, c.g() as f32, c.b() as f32])
}

/// RGBA pixels (straight alpha) for a `size` x `size` window icon, 4x4 supersampled.
pub fn icon_rgba(size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    let n = 4u32;
    for py in 0..size {
        for px in 0..size {
            let (mut rgb, mut covered) = ([0.0f32; 3], 0.0f32);
            for sy in 0..n {
                for sx in 0..n {
                    let x = (px as f32 + (sx as f32 + 0.5) / n as f32) / size as f32 * 28.0;
                    let y = (py as f32 + (sy as f32 + 0.5) / n as f32) / size as f32 * 28.0;
                    if let Some(c) = sample(x, y) {
                        covered += 1.0;
                        for k in 0..3 {
                            rgb[k] += c[k];
                        }
                    }
                }
            }
            let a = (covered / (n * n) as f32 * 255.0).round() as u8;
            let d = covered.max(1.0);
            out.extend_from_slice(&[
                (rgb[0] / d).round() as u8,
                (rgb[1] / d).round() as u8,
                (rgb[2] / d).round() as u8,
                a,
            ]);
        }
    }
    out
}

/// Paints the mark `size` points wide.
pub fn paint(ui: &mut Ui, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let k = size / 28.0;
    let at = |x: f32, y: f32| Pos2::new(rect.min.x + x * k, rect.min.y + y * k);
    let p = ui.painter();
    p.rect(
        rect,
        BADGE_ROUND * k,
        DARK,
        Stroke::new(1.0, Color32::from_white_alpha(36)),
        egui::StrokeKind::Inside,
    );
    p.add(egui::Shape::convex_polygon(
        HEX.iter().map(|&(x, y)| at(x, y)).collect(),
        WHITE,
        Stroke::NONE,
    ));
    for (x0, y0, x1, y1) in BARS {
        p.rect_filled(Rect::from_min_max(at(x0, y0), at(x1, y1)), 0.0, DARK);
    }
    p.circle_filled(at(14.0, 14.0), DOT_R * k, ACCENT);
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
            &[0xff, 0x4d, 0x5e],
            "the centre dot is red"
        );
        assert_eq!(
            &at(24, 24)[..3],
            &[0xf6, 0xf6, 0xf8],
            "the hexagon is white"
        );
        assert_eq!(
            &at(32, 20)[..3],
            &[0x0b, 0x0b, 0x0d],
            "the crosshair bar is cut out of it"
        );
        assert_eq!(
            &at(4, 32)[..3],
            &[0x0b, 0x0b, 0x0d],
            "the badge itself is dark"
        );
    }
}
