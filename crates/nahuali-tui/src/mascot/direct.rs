//! Kitty direct placement, for terminals that draw the graphics protocol but
//! not its Unicode placeholders (Tern).
//!
//! The image lives outside the cell grid, so nothing goes through ratatui:
//! the painter records where the sprite belongs and, once the frame is
//! flushed, [`Direct`] turns that into commands inside the same synchronized
//! update. Each frame is transmitted once; after that a frame change is one
//! placement and one deletion.
//!
//! What Tern 0.9.12 was seen to do, and this module relies on:
//!
//! - `a=t` then `a=p` draws at the cursor, scaled to `c`×`r` cells, and
//!   placing the same image and placement id again moves it.
//! - `d=i` removes placements and keeps the data; `d=i` with `p` removes one.
//! - Clearing the screen, `d=a` and switching screens free the data too, so
//!   a resize (ratatui clears) means transmitting again.
//! - A placement survives text written over its cells, and it hides that
//!   text: `z` below zero goes under the cell backgrounds, where the opaque
//!   theme hides the image. Anything drawn over a sprite must remove it.

use std::collections::HashMap;
use std::io::{Cursor, Write};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::imageops::{self, FilterType};
use image::{ImageFormat, RgbaImage};
use nahuali_view::MascotState;
use ratatui::layout::{Rect, Size};

/// Largest payload of one escape sequence; a multiple of 4, as the protocol
/// asks of every chunk but the last.
const CHUNK: usize = 4096;
/// Above the text, which paints nothing under the sprite.
const Z_INDEX: u8 = 1;
/// Cell size when the terminal does not report pixels.
pub const FALLBACK_CELL: Size = Size::new(10, 20);

/// One frame at one size in cells.
pub type FrameKey = (MascotState, usize, Size);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shown {
    id: u32,
    slot: u32,
    rect: Rect,
}

/// The images the terminal holds and what is on screen.
#[derive(Debug)]
pub struct Direct {
    cell: Size,
    screen: Option<Size>,
    images: HashMap<FrameKey, u32>,
    shown: Vec<Shown>,
    next_id: u32,
}

impl Direct {
    /// `cell` is the cell size in pixels.
    pub fn new(cell: Size) -> Self {
        Self { cell, screen: None, images: HashMap::new(), shown: Vec::new(), next_id: 1 }
    }

    pub fn cell(&self) -> Size {
        self.cell
    }

    /// A new font size: every image has the wrong resolution.
    pub fn set_cell(&mut self, cell: Size) {
        if cell != self.cell {
            self.cell = cell;
            self.screen = None;
        }
    }

    /// Start a frame. A different screen means ratatui just cleared it,
    /// which took the images with it on Tern; elsewhere they are freed here.
    pub fn begin(&mut self, screen: Size, out: &mut Vec<u8>) {
        if self.screen != Some(screen) {
            self.free_all(out);
            self.screen = Some(screen);
        }
    }

    /// The id of a frame, transmitting it the first time it is asked for.
    pub fn image(
        &mut self,
        key: FrameKey,
        png: impl FnOnce() -> Option<Vec<u8>>,
        out: &mut Vec<u8>,
    ) -> Option<u32> {
        if let Some(id) = self.images.get(&key) {
            return Some(*id);
        }
        let png = png()?;
        let id = self.next_id;
        self.next_id += 1;
        transmit(id, &png, out);
        self.images.insert(key, id);
        Some(id)
    }

    /// Make the screen show exactly `wanted`. The new placement goes first so
    /// that a terminal drawing between commands never shows an empty cell.
    pub fn show(&mut self, wanted: &[(u32, Rect)], out: &mut Vec<u8>) {
        let next: Vec<Shown> =
            (1..).zip(wanted).map(|(slot, &(id, rect))| Shown { id, slot, rect }).collect();
        for placement in &next {
            if !self.shown.contains(placement) {
                place(placement, out);
            }
        }
        for old in &self.shown {
            if !next.iter().any(|new| new.id == old.id && new.slot == old.slot) {
                remove(old, out);
            }
        }
        self.shown = next;
    }

    /// Free every image, for the end of the session.
    pub fn release(&mut self) -> Vec<u8> {
        let mut out = Vec::new();
        self.free_all(&mut out);
        out
    }

    fn free_all(&mut self, out: &mut Vec<u8>) {
        for id in self.images.values() {
            let _ = write!(out, "\x1b_Ga=d,d=I,i={id},q=2\x1b\\");
        }
        self.images.clear();
        self.shown.clear();
    }
}

/// `source` as a PNG of exactly `size` cells: scaled to fit, centred on
/// transparency, so the terminal has nothing left to scale.
pub fn frame_png(source: &RgbaImage, size: Size, cell: Size) -> Option<Vec<u8>> {
    let (width, height) =
        (u32::from(size.width) * u32::from(cell.width), u32::from(size.height) * u32::from(cell.height));
    let scale = (width as f32 / source.width() as f32).min(height as f32 / source.height() as f32);
    let scaled_width = ((source.width() as f32 * scale).round() as u32).clamp(1, width.max(1));
    let scaled_height = ((source.height() as f32 * scale).round() as u32).clamp(1, height.max(1));
    let scaled = imageops::resize(source, scaled_width, scaled_height, FilterType::Lanczos3);
    let mut canvas = RgbaImage::new(width.max(1), height.max(1));
    let (x, y) = ((width - scaled_width) / 2, (height - scaled_height) / 2);
    imageops::replace(&mut canvas, &scaled, i64::from(x), i64::from(y));
    let mut png = Vec::new();
    canvas.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).ok()?;
    Some(png)
}

fn transmit(id: u32, png: &[u8], out: &mut Vec<u8>) {
    let encoded = STANDARD.encode(png);
    let mut chunks = encoded.as_bytes().chunks(CHUNK).peekable();
    let mut first = true;
    while let Some(chunk) = chunks.next() {
        let more = u8::from(chunks.peek().is_some());
        if first {
            let _ = write!(out, "\x1b_Ga=t,f=100,i={id},q=2,m={more};");
            first = false;
        } else {
            let _ = write!(out, "\x1b_Gq=2,m={more};");
        }
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
}

/// `C=1` leaves the cursor alone; the save and restore keep ratatui's idea of
/// it right even on a terminal that ignores that.
fn place(shown: &Shown, out: &mut Vec<u8>) {
    let Shown { id, slot, rect } = shown;
    let _ = write!(
        out,
        "\x1b7\x1b[{};{}H\x1b_Ga=p,i={id},p={slot},c={},r={},C=1,q=2,z={Z_INDEX}\x1b\\\x1b8",
        rect.y + 1,
        rect.x + 1,
        rect.width,
        rect.height,
    );
}

fn remove(shown: &Shown, out: &mut Vec<u8>) {
    let Shown { id, slot, .. } = shown;
    let _ = write!(out, "\x1b_Ga=d,d=i,i={id},p={slot},q=2\x1b\\");
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Size = Size::new(120, 36);
    const RECT: Rect = Rect::new(2, 3, 26, 13);

    fn key(frame: usize) -> FrameKey {
        (MascotState::Idle, frame, RECT.as_size())
    }

    /// Run one frame showing `frame` at `rect`; returns the commands sent.
    fn frame(direct: &mut Direct, frame: usize, rect: Rect) -> String {
        let mut out = Vec::new();
        direct.begin(SCREEN, &mut out);
        let id = direct.image(key(frame), || Some(vec![1, 2, 3]), &mut out).unwrap();
        direct.show(&[(id, rect)], &mut out);
        String::from_utf8(out).unwrap()
    }

    fn direct() -> Direct {
        Direct::new(FALLBACK_CELL)
    }

    #[test]
    fn a_large_image_is_chunked_as_the_protocol_asks() {
        let mut out = Vec::new();
        transmit(7, &vec![0_u8; 10_000], &mut out);
        let text = String::from_utf8(out).unwrap();
        let commands: Vec<&str> = text.split("\x1b\\").filter(|c| !c.is_empty()).collect();
        assert_eq!(commands.len(), 4, "13336 base64 bytes in 4096-byte chunks");
        assert!(commands[0].starts_with("\x1b_Ga=t,f=100,i=7,q=2,m=1;"));
        for command in &commands[1..3] {
            assert!(command.starts_with("\x1b_Gq=2,m=1;"), "later chunks carry only m and q");
        }
        assert!(commands[3].starts_with("\x1b_Gq=2,m=0;"));
        let payloads: Vec<&str> = commands.iter().map(|c| c.split_once(';').unwrap().1).collect();
        assert!(payloads[..3].iter().all(|p| p.len() == CHUNK));
    }

    #[test]
    fn a_small_image_is_a_single_final_chunk() {
        let mut out = Vec::new();
        transmit(1, &[9, 9, 9], &mut out);
        assert_eq!(String::from_utf8(out).unwrap(), "\x1b_Ga=t,f=100,i=1,q=2,m=0;CQkJ\x1b\\");
    }

    #[test]
    fn a_placement_moves_the_cursor_there_and_back() {
        let mut direct = direct();
        let sent = frame(&mut direct, 0, RECT);
        assert!(sent.ends_with("\x1b7\x1b[4;3H\x1b_Ga=p,i=1,p=1,c=26,r=13,C=1,q=2,z=1\x1b\\\x1b8"));
    }

    #[test]
    fn a_frame_is_transmitted_once_and_an_unchanged_frame_sends_nothing() {
        let mut direct = direct();
        assert!(frame(&mut direct, 0, RECT).contains("a=t"));
        assert_eq!(frame(&mut direct, 0, RECT), "");
        assert!(frame(&mut direct, 1, RECT).contains("a=t"));
        let back = frame(&mut direct, 0, RECT);
        assert!(!back.contains("a=t"), "frame 0 is resident: {back:?}");
        assert!(back.contains("a=p,i=1"));
    }

    #[test]
    fn a_new_frame_is_placed_before_the_old_one_goes() {
        let mut direct = direct();
        frame(&mut direct, 0, RECT);
        let swap = frame(&mut direct, 1, RECT);
        let placed = swap.find("a=p,i=2").expect("new placement");
        let removed = swap.find("a=d,d=i,i=1,p=1").expect("old placement removed");
        assert!(placed < removed);
    }

    #[test]
    fn moving_a_frame_replaces_its_placement_without_a_deletion() {
        let mut direct = direct();
        frame(&mut direct, 0, RECT);
        let moved = frame(&mut direct, 0, Rect::new(10, 3, 26, 13));
        assert!(moved.contains("a=p,i=1,p=1"));
        assert!(!moved.contains("a=d"));
    }

    #[test]
    fn a_sprite_that_leaves_the_screen_is_removed_and_stays_resident() {
        let mut direct = direct();
        frame(&mut direct, 0, RECT);
        let mut out = Vec::new();
        direct.show(&[], &mut out);
        assert_eq!(String::from_utf8(out).unwrap(), "\x1b_Ga=d,d=i,i=1,p=1,q=2\x1b\\");
        let again = frame(&mut direct, 0, RECT);
        assert!(!again.contains("a=t") && again.contains("a=p,i=1"));
    }

    #[test]
    fn a_resize_frees_everything_and_transmits_again() {
        let mut direct = direct();
        frame(&mut direct, 0, RECT);
        frame(&mut direct, 1, RECT);
        let mut out = Vec::new();
        direct.begin(Size::new(100, 30), &mut out);
        let sent = String::from_utf8(out).unwrap();
        assert!(sent.contains("d=I,i=1") && sent.contains("d=I,i=2"));

        let mut out = Vec::new();
        direct.begin(Size::new(100, 30), &mut out);
        assert!(out.is_empty(), "same screen: nothing to forget");
        let id = direct.image(key(0), || Some(vec![1]), &mut out).unwrap();
        direct.show(&[(id, RECT)], &mut out);
        let sent = String::from_utf8(out).unwrap();
        assert!(sent.contains("a=t") && sent.contains("a=p"), "{sent:?}");
        assert!(!sent.contains("a=d"), "nothing is left to delete: {sent:?}");
    }

    #[test]
    fn a_new_cell_size_makes_every_image_stale() {
        let mut direct = direct();
        frame(&mut direct, 0, RECT);
        direct.set_cell(FALLBACK_CELL);
        assert_eq!(frame(&mut direct, 0, RECT), "", "same cell: nothing changes");
        direct.set_cell(Size::new(18, 41));
        let sent = frame(&mut direct, 0, RECT);
        assert!(sent.contains("d=I,i=1") && sent.contains("a=t,f=100,i=2"), "{sent:?}");
    }

    #[test]
    fn release_frees_what_was_transmitted() {
        let mut direct = direct();
        frame(&mut direct, 0, RECT);
        assert_eq!(String::from_utf8(direct.release()).unwrap(), "\x1b_Ga=d,d=I,i=1,q=2\x1b\\");
        assert!(direct.release().is_empty());
    }

    #[test]
    fn a_frame_png_has_the_exact_pixels_of_its_cells_and_keeps_alpha() {
        let source = RgbaImage::from_pixel(64, 64, image::Rgba([200, 100, 100, 255]));
        let png = frame_png(&source, Size::new(26, 13), Size::new(18, 41)).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (468, 533));
        assert_eq!(decoded.get_pixel(234, 0)[3], 0, "letterboxed with transparency");
        assert_eq!(decoded.get_pixel(234, 266)[3], 255);
    }
}
