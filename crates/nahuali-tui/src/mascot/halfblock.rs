//! The fallback for terminals without an image protocol: two pixels per cell
//! with `▀`/`▄`, quantised to a hand-picked palette of the axolotl's own
//! colours. Each target pixel takes the mean colour of its source box,
//! except where dark pixels dominate the box, so the eyes survive the
//! downscale and the thin outline does not swallow the gills. On 256-colour terminals every palette entry
//! has a hand-chosen xterm index, never a monochrome silhouette.

use image::RgbaImage;
use palette::{FromColor, Oklab, Srgb};
use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;

use crate::theme::{ColorMode, Rgb};

/// One palette entry: colour and its xterm-256 stand-in.
#[derive(Clone, Copy, Debug)]
struct Ink {
    rgb: Rgb,
    xterm: u8,
}

/// Sampled from `assets/mascot/v1/spritesheet.png`, outline lifted to a warm
/// brown so it reads on the coffee background.
const PALETTE: [Ink; 9] = [
    Ink { rgb: Rgb(84, 34, 28), xterm: 52 },     // outline, pupils
    Ink { rgb: Rgb(206, 76, 72), xterm: 167 },   // gill shadow
    Ink { rgb: Rgb(239, 111, 103), xterm: 203 }, // gill
    Ink { rgb: Rgb(245, 156, 146), xterm: 210 }, // gill light
    Ink { rgb: Rgb(246, 176, 164), xterm: 217 }, // blush, speckles
    Ink { rgb: Rgb(242, 203, 190), xterm: 223 }, // body shade
    Ink { rgb: Rgb(252, 226, 216), xterm: 224 }, // body
    Ink { rgb: Rgb(255, 241, 227), xterm: 230 }, // belly
    Ink { rgb: Rgb(255, 255, 255), xterm: 231 }, // eye glint
];

const OPAQUE: u8 = 110;
/// Share of dark source pixels that turns a target pixel dark.
const DARK_SHARE: f32 = 0.4;

fn lab(rgb: (u8, u8, u8)) -> Oklab {
    Oklab::from_color(Srgb::new(rgb.0, rgb.1, rgb.2).into_format::<f32>())
}

/// Index of the nearest palette entry in OKLab.
fn nearest(labs: &[Oklab; 9], rgb: (u8, u8, u8)) -> usize {
    let c = lab(rgb);
    let mut best = (0, f32::MAX);
    for (i, p) in labs.iter().enumerate() {
        let d = (p.l - c.l).powi(2) + (p.a - c.a).powi(2) + (p.b - c.b).powi(2);
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// Index of the nearest palette entry, outline and glint excluded.
fn nearest_light(labs: &[Oklab; 9], rgb: (u8, u8, u8)) -> usize {
    let c = lab(rgb);
    let mut best = (1, f32::MAX);
    for (i, p) in labs.iter().enumerate().take(8).skip(1) {
        let d = (p.l - c.l).powi(2) + (p.a - c.a).powi(2) + (p.b - c.b).powi(2);
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// A sprite frame precomputed as cells.
#[derive(Clone, Debug)]
pub struct HalfSprite {
    pub size: Size,
    cells: Vec<Option<(&'static str, Color, Color)>>,
}

impl HalfSprite {
    /// Quantise `image` into `size` cells (`size.height * 2` pixel rows).
    pub fn new(image: &RgbaImage, size: Size, mode: ColorMode, bg: Color) -> Self {
        let labs = PALETTE.map(|ink| lab((ink.rgb.0, ink.rgb.1, ink.rgb.2)));
        // Four source pixels per target pixel are plenty for the box vote;
        // shrinking first keeps the OKLab pass cheap.
        let (max_w, max_h) = (u32::from(size.width) * 4, u32::from(size.height) * 8);
        let shrunk;
        let image = if image.width() > max_w && image.height() > max_h {
            shrunk = image::imageops::resize(image, max_w, max_h, image::imageops::FilterType::Triangle);
            &shrunk
        } else {
            image
        };
        // Classify every source pixel once.
        let classes: Vec<Option<usize>> = image
            .pixels()
            .map(|p| (p.0[3] >= OPAQUE).then(|| nearest(&labs, (p.0[0], p.0[1], p.0[2]))))
            .collect();
        let (sw, sh) = (image.width() as usize, image.height() as usize);
        let (tw, th) = (usize::from(size.width), usize::from(size.height) * 2);
        let mut pixels: Vec<Option<usize>> = Vec::with_capacity(tw * th);
        for ty in 0..th {
            let y0 = ty * sh / th;
            let y1 = ((ty + 1) * sh / th).max(y0 + 1).min(sh);
            for tx in 0..tw {
                let x0 = tx * sw / tw;
                let x1 = ((tx + 1) * sw / tw).max(x0 + 1).min(sw);
                let mut sum = [0f32; 3];
                let mut opaque = 0f32;
                let mut clear = 0f32;
                let mut counts = [0f32; 9];
                for y in y0..y1 {
                    for x in x0..x1 {
                        match classes[y * sw + x] {
                            Some(i) => {
                                let p = image.get_pixel(x as u32, y as u32).0;
                                sum[0] += f32::from(p[0]);
                                sum[1] += f32::from(p[1]);
                                sum[2] += f32::from(p[2]);
                                opaque += 1.0;
                                counts[i] += 1.0;
                            }
                            None => clear += 1.0,
                        }
                    }
                }
                if opaque == 0.0 || clear / (clear + opaque) > 0.55 {
                    pixels.push(None);
                    continue;
                }
                // Dark features (eyes, mouth) win when they dominate the box;
                // otherwise the box's mean colour picks a light ink, so thin
                // outlines do not swallow the gills.
                if counts[0] / opaque >= DARK_SHARE {
                    pixels.push(Some(0));
                    continue;
                }
                if counts[8] / opaque >= 0.3 {
                    pixels.push(Some(8));
                    continue;
                }
                let mean = (
                    (sum[0] / opaque).round() as u8,
                    (sum[1] / opaque).round() as u8,
                    (sum[2] / opaque).round() as u8,
                );
                pixels.push(Some(nearest_light(&labs, mean)));
            }
        }
        let color = |i: usize| match mode {
            ColorMode::TrueColor => {
                let Rgb(r, g, b) = PALETTE[i].rgb;
                Color::Rgb(r, g, b)
            }
            ColorMode::Ansi256 => Color::Indexed(PALETTE[i].xterm),
        };
        let mut cells = Vec::with_capacity(usize::from(size.width * size.height));
        for y in 0..usize::from(size.height) {
            for x in 0..tw {
                let top = pixels[(2 * y) * tw + x];
                let bottom = pixels[(2 * y + 1) * tw + x];
                cells.push(match (top, bottom) {
                    (None, None) => None,
                    (Some(t), None) => Some(("▀", color(t), bg)),
                    (None, Some(b)) => Some(("▄", color(b), bg)),
                    (Some(t), Some(b)) if t == b => Some(("█", color(t), color(b))),
                    (Some(t), Some(b)) => Some(("▀", color(t), color(b))),
                });
            }
        }
        Self { size, cells }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        for (index, cell) in self.cells.iter().enumerate() {
            let x = (index % usize::from(self.size.width)) as u16;
            let y = (index / usize::from(self.size.width)) as u16;
            if x >= area.width || y >= area.height {
                continue;
            }
            let Some((symbol, fg, bg)) = cell else {
                continue;
            };
            if let Some(target) = buf.cell_mut((area.x + x, area.y + y)) {
                target.set_symbol(symbol).set_fg(*fg).set_bg(*bg);
            }
        }
    }

    /// Visible cells, for tests.
    pub fn visible(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// Distinct foreground colours, for tests.
    pub fn colours(&self) -> usize {
        let mut seen: Vec<Color> = Vec::new();
        for (_, fg, bg) in self.cells.iter().flatten() {
            for c in [fg, bg] {
                if !seen.contains(c) {
                    seen.push(*c);
                }
            }
        }
        seen.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mascot::atlas::Atlas;
    use nahuali_view::MascotState;

    #[test]
    fn quantised_sprite_keeps_colour_and_shape() {
        let atlas = Atlas::embedded();
        let image = &atlas.frames(MascotState::Idle)[0].image;
        let sprite = HalfSprite::new(image, Size::new(26, 13), ColorMode::TrueColor, Color::Black);
        let area = 26 * 13;
        assert!(sprite.visible() > area / 3, "visible {}", sprite.visible());
        assert!(sprite.visible() < area, "corners stay transparent");
        assert!(sprite.colours() >= 5, "not monochrome: {}", sprite.colours());
    }

    #[test]
    fn ansi256_uses_hand_picked_indices() {
        let atlas = Atlas::embedded();
        let image = &atlas.frames(MascotState::Worried)[0].image;
        let sprite = HalfSprite::new(image, Size::new(12, 6), ColorMode::Ansi256, Color::Black);
        let xterms: Vec<u8> = PALETTE.iter().map(|i| i.xterm).collect();
        for (_, fg, _) in sprite.cells.iter().flatten() {
            match fg {
                Color::Indexed(i) => assert!(xterms.contains(i)),
                other => panic!("unexpected {other:?}"),
            }
        }
    }
}
