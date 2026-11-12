//! The animated axolotl: frames from the atlas, drawn through the best
//! protocol the terminal speaks.
//!
//! - Kitty (Ghostty, WezTerm via iTerm2, Kitty): every frame becomes a
//!   `ratatui-image` protocol once per size; it is transmitted the first time
//!   it is shown and afterwards drawn with Unicode placeholders, so switching
//!   frames only recolours cells.
//! - Kitty without placeholders (Tern): the same single transmission, then
//!   each frame is placed at the sprite's cells ([`direct`]).
//! - iTerm2: frames are resent whole on every change, so the frame floor
//!   rises to 250 ms (4 fps).
//! - Anything else: quantised half blocks ([`halfblock`]).

pub mod ansi;
pub mod atlas;
pub mod direct;
pub mod halfblock;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use image::{DynamicImage, RgbaImage};
use nahuali_view::MascotState;
use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;
use ratatui::widgets::Widget;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::Protocol;
use ratatui_image::{Image, Resize};

use crate::anim::{Clock, MascotAnimator, Motion, Timing};
use crate::render::SpritePainter;
use crate::theme::ColorMode;
use atlas::Atlas;
use direct::Direct;
use halfblock::HalfSprite;

/// How sprites reach the terminal.
pub enum Backend {
    Halfblocks,
    Direct(Direct),
    Raster(Box<Picker>),
}

impl Backend {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Halfblocks => "halfblocks",
            Self::Direct(_) => "kitty-direct",
            Self::Raster(p) => match p.protocol_type() {
                ProtocolType::Kitty => "kitty",
                ProtocolType::Iterm2 => "iterm2",
                ProtocolType::Sixel => "sixel",
                ProtocolType::Halfblocks => "halfblocks",
            },
        }
    }
}

enum Rendered {
    Half(HalfSprite),
    Raster(Protocol),
}

/// Where a sprite was drawn on the last frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub rect: Rect,
    pub state: MascotState,
    pub frame: usize,
}

/// Atlas + backend + animator + per-size cache.
pub struct Sprites {
    atlas: Atlas,
    backend: Backend,
    mode: ColorMode,
    bg: Color,
    cache: HashMap<(MascotState, usize, Size), Rendered>,
    pub animator: MascotAnimator,
    placements: Vec<Placement>,
    forced_frame: Option<usize>,
}

impl Sprites {
    pub fn new(
        atlas: Atlas,
        backend: Backend,
        mode: ColorMode,
        bg: Color,
        motion: Motion,
        clock: Clock,
        now: Instant,
    ) -> Self {
        let mut animator = MascotAnimator::new(clock, motion, MascotState::Greeting, now);
        if let Backend::Raster(picker) = &backend
            && picker.protocol_type() != ProtocolType::Kitty
        {
            animator.set_floor(Duration::from_millis(250));
        }
        let mut sprites = Self {
            atlas,
            backend,
            mode,
            bg,
            cache: HashMap::new(),
            animator,
            placements: Vec::new(),
            forced_frame: None,
        };
        let timings = sprites.timings(sprites.animator.shown());
        sprites.animator.retime(now, &timings);
        sprites
    }

    /// Deterministic half-block sprites with the embedded art, for tests.
    pub fn for_tests(mode: ColorMode, bg: Color) -> Self {
        Self::for_tests_with(Backend::Halfblocks, mode, bg)
    }

    /// A Kitty backend that never queries the terminal (10×20 px cells),
    /// for tests and screenshots of the image-protocol path.
    pub fn kitty_for_tests(mode: ColorMode, bg: Color) -> Self {
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Kitty);
        Self::for_tests_with(Backend::Raster(Box::new(picker)), mode, bg)
    }

    /// A direct-placement backend (10×20 px cells), for tests.
    pub fn direct_for_tests(mode: ColorMode, bg: Color) -> Self {
        Self::for_tests_with(Backend::Direct(Direct::new(direct::FALLBACK_CELL)), mode, bg)
    }

    fn for_tests_with(backend: Backend, mode: ColorMode, bg: Color) -> Self {
        let now = Instant::now();
        let mut sprites = Self::new(Atlas::embedded(), backend, mode, bg, Motion::None, Clock::new(now), now);
        sprites.animator = MascotAnimator::new(Clock::new(now), Motion::None, MascotState::Idle, now);
        sprites
    }

    pub fn backend(&self) -> &Backend {
        &self.backend
    }

    pub fn atlas(&self) -> &Atlas {
        &self.atlas
    }

    pub fn timings(&self, state: MascotState) -> Vec<Timing> {
        self.atlas.frames(state).iter().map(|f| Timing { ms: f.ms, jitter_ms: f.jitter_ms }).collect()
    }

    /// Request a state and advance time. Returns true if the sprite changed.
    pub fn update(&mut self, target: MascotState, now: Instant) -> bool {
        let timings = self.timings(self.animator.shown());
        let mut changed = self.animator.request(target, now, &timings);
        changed |= self.animator.tick(now, &timings);
        if changed {
            let timings = self.timings(self.animator.shown());
            self.animator.retime(now, &timings);
        }
        changed
    }

    pub fn set_focus(&mut self, focused: bool, now: Instant) {
        let timings = self.timings(self.animator.shown());
        self.animator.set_focus(focused, now, &timings);
    }

    /// Show `state` immediately, ignoring the dwell (tests and screenshots).
    pub fn force(&mut self, state: MascotState, frame: usize) {
        let now = Instant::now();
        self.animator = MascotAnimator::new(Clock::new(now), Motion::None, state, now);
        self.forced_frame = Some(frame);
    }

    /// Placements recorded by the last draw.
    pub fn placements(&self) -> &[Placement] {
        &self.placements
    }

    pub fn clear_placements(&mut self) {
        self.placements.clear();
    }

    /// The cell size in pixels changed (direct placement renders to it).
    pub fn set_cell(&mut self, cell: Size) {
        if let Backend::Direct(direct) = &mut self.backend {
            direct.set_cell(cell);
        }
    }

    /// What the terminal needs to show the last draw's placements, to be
    /// written after that draw is flushed. Empty unless placing directly.
    pub fn direct_commands(&mut self, screen: Size) -> Vec<u8> {
        let Backend::Direct(direct) = &mut self.backend else { return Vec::new() };
        let mut out = Vec::new();
        direct.begin(screen, &mut out);
        let cell = direct.cell();
        let mut wanted = Vec::with_capacity(self.placements.len());
        for placement in &self.placements {
            let frames = self.atlas.frames(placement.state);
            let source = &frames[placement.frame % frames.len()].image;
            let size = placement.rect.as_size();
            let key = (placement.state, placement.frame, size);
            let id = direct.image(key, || direct::frame_png(source, size, cell), &mut out);
            wanted.extend(id.map(|id| (id, placement.rect)));
        }
        direct.show(&wanted, &mut out);
        out
    }

    /// What frees the terminal's copies of the sprites, before leaving.
    pub fn release(&mut self) -> Vec<u8> {
        match &mut self.backend {
            Backend::Direct(direct) => direct.release(),
            _ => Vec::new(),
        }
    }

    /// The source image of a frame (for compositing screenshots).
    pub fn image(&self, state: MascotState, frame: usize) -> &RgbaImage {
        let frames = self.atlas.frames(state);
        &frames[frame % frames.len()].image
    }

    /// Cell height / cell width of the terminal font.
    fn cell_ratio(&self) -> f32 {
        match &self.backend {
            Backend::Halfblocks => 2.0,
            Backend::Direct(direct) => {
                let cell = direct.cell();
                f32::from(cell.height.max(1)) / f32::from(cell.width.max(1))
            }
            Backend::Raster(picker) => {
                let size = picker.font_size();
                f32::from(size.height.max(1)) / f32::from(size.width.max(1))
            }
        }
    }

    fn current(&self) -> (MascotState, usize) {
        let state = self.animator.shown();
        let count = self.atlas.frames(state).len().max(1);
        let frame = self.forced_frame.unwrap_or(self.animator.frame()) % count;
        (state, frame)
    }

    fn rendered(&mut self, state: MascotState, frame: usize, size: Size) -> Option<&Rendered> {
        let key = (state, frame, size);
        if !self.cache.contains_key(&key) {
            // Keep a few placement sizes (sidebar, band); a resize storm must
            // not grow the cache without bound.
            let mut sizes: Vec<Size> = self.cache.keys().map(|k| k.2).collect();
            sizes.sort_by_key(|s| (s.width, s.height));
            sizes.dedup();
            if sizes.len() >= 3 && !sizes.contains(&size) {
                self.cache.clear();
            }
            let image = self.image(state, frame).clone();
            let rendered = match &self.backend {
                Backend::Halfblocks => Rendered::Half(HalfSprite::new(&image, size, self.mode, self.bg)),
                Backend::Direct(_) => return None,
                Backend::Raster(picker) => {
                    let protocol =
                        picker.new_protocol(DynamicImage::ImageRgba8(image), size, Resize::Fit(None)).ok()?;
                    Rendered::Raster(protocol)
                }
            };
            self.cache.insert(key, rendered);
        }
        self.cache.get(&key)
    }
}

impl SpritePainter for Sprites {
    fn fit(&self, max: Size) -> Size {
        let aspect = self.atlas.aspect() * self.cell_ratio();
        let by_width = (f32::from(max.width) / aspect).floor() as u16;
        let height = max.height.min(by_width).max(1);
        let width = ((f32::from(height) * aspect).round() as u16).clamp(1, max.width.max(1));
        Size::new(width, height)
    }

    fn paint(&mut self, area: Rect, buf: &mut Buffer) {
        let size = self.fit(area.as_size());
        let rect =
            Rect::new(area.x + area.width.saturating_sub(size.width) / 2, area.y, size.width, size.height);
        let (state, frame) = self.current();
        // Direct placement leaves the cells alone; see `direct_commands`.
        if !matches!(self.backend, Backend::Direct(_)) {
            match self.rendered(state, frame, size) {
                Some(Rendered::Half(sprite)) => sprite.render(rect, buf),
                Some(Rendered::Raster(protocol)) => Image::new(protocol).render(rect, buf),
                None => return,
            }
        }
        self.placements.push(Placement { rect, state, frame });
    }

    fn occlude(&mut self, rect: Rect) {
        if matches!(self.backend, Backend::Direct(_)) {
            self.placements.retain(|placement| !placement.rect.intersects(rect));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::SpritePainter;

    const PLACEHOLDER: char = '\u{10EEEE}';

    fn render(sprites: &mut Sprites) -> Buffer {
        let area = Rect::new(0, 0, 30, 14);
        let mut buf = Buffer::empty(area);
        sprites.paint(area, &mut buf);
        buf
    }

    fn transmits(buf: &Buffer) -> usize {
        buf.content().iter().filter(|c| c.symbol().contains("\x1b_G")).count()
    }

    #[test]
    fn kitty_transmits_each_frame_once_then_only_switches_placeholders() {
        let mut sprites = Sprites::kitty_for_tests(ColorMode::TrueColor, Color::Black);
        sprites.force(MascotState::Idle, 0);
        let first = render(&mut sprites);
        assert_eq!(transmits(&first), 1, "first show transmits the frame");
        assert!(first.content().iter().any(|c| c.symbol().contains(PLACEHOLDER)));
        let again = render(&mut sprites);
        assert_eq!(transmits(&again), 0, "same frame again: placeholders only");

        sprites.force(MascotState::Idle, 1);
        let other = render(&mut sprites);
        assert_eq!(transmits(&other), 1, "a new frame is transmitted once");
        // Switching frames only recolours placeholders: the id lives in fg.
        let fg = |b: &Buffer| b.content().iter().find(|c| c.symbol().contains(PLACEHOLDER)).map(|c| c.fg);
        assert_ne!(fg(&again), fg(&other));

        sprites.force(MascotState::Idle, 0);
        assert_eq!(transmits(&render(&mut sprites)), 0, "frame 0 is still resident");
    }

    #[test]
    fn direct_placement_leaves_the_cells_blank_and_places_after_the_draw() {
        let mut sprites = Sprites::direct_for_tests(ColorMode::TrueColor, Color::Black);
        sprites.force(MascotState::Idle, 0);
        let buf = render(&mut sprites);
        assert!(buf.content().iter().all(|c| c.symbol() == " "), "nothing in the grid");
        let placement = sprites.placements()[0];
        let sent = String::from_utf8_lossy(&sprites.direct_commands(Size::new(30, 14))).into_owned();
        assert!(sent.starts_with("\x1b_Ga=t,f=100,i=1,q=2,"), "transmits first");
        let expected = format!("a=p,i=1,p=1,c={},r={}", placement.rect.width, placement.rect.height);
        assert!(sent.contains(&expected), "{expected}");
    }

    #[test]
    fn direct_placement_removes_a_sprite_the_next_draw_does_not_paint() {
        let mut sprites = Sprites::direct_for_tests(ColorMode::TrueColor, Color::Black);
        sprites.force(MascotState::Idle, 0);
        render(&mut sprites);
        sprites.direct_commands(Size::new(30, 14));
        sprites.clear_placements();
        let sent = String::from_utf8(sprites.direct_commands(Size::new(30, 14))).unwrap();
        assert_eq!(sent, "\x1b_Ga=d,d=i,i=1,p=1,q=2\x1b\\");
    }

    #[test]
    fn only_direct_placement_gives_up_a_sprite_something_covers() {
        let covers = Rect::new(0, 0, 30, 14);
        let mut direct = Sprites::direct_for_tests(ColorMode::TrueColor, Color::Black);
        render(&mut direct);
        direct.occlude(Rect::new(40, 20, 5, 5));
        assert_eq!(direct.placements().len(), 1, "elsewhere on screen");
        direct.occlude(covers);
        assert!(direct.placements().is_empty());

        let mut half = Sprites::for_tests(ColorMode::TrueColor, Color::Black);
        render(&mut half);
        half.occlude(covers);
        assert_eq!(half.placements().len(), 1, "the overlay's cells replace it anyway");
    }

    #[test]
    fn direct_placement_fits_the_reported_cell_shape() {
        let mut sprites = Sprites::direct_for_tests(ColorMode::TrueColor, Color::Black);
        let max = Size::new(60, 13);
        let plain = sprites.fit(max);
        sprites.set_cell(Size::new(18, 41));
        let tall_cells = sprites.fit(max);
        assert_eq!((plain.height, tall_cells.height), (13, 13));
        assert!(tall_cells.width > plain.width, "{plain:?} {tall_cells:?}");
    }

    #[test]
    fn iterm2_frames_are_capped_at_four_per_second() {
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Iterm2);
        let now = Instant::now();
        let sprites = Sprites::new(
            Atlas::embedded(),
            Backend::Raster(Box::new(picker)),
            ColorMode::TrueColor,
            Color::Black,
            Motion::Full,
            Clock::new(now),
            now,
        );
        let deadline = sprites.animator.next_deadline().expect("animating");
        assert!(deadline - now >= Duration::from_millis(250));
    }

    #[test]
    fn halfblock_sprite_fits_the_cells_it_is_given() {
        let mut sprites = Sprites::for_tests(ColorMode::TrueColor, Color::Black);
        for max in [Size::new(26, 13), Size::new(16, 6), Size::new(12, 6)] {
            let size = sprites.fit(max);
            assert!(size.width <= max.width && size.height <= max.height, "{size:?} in {max:?}");
        }
        let buf = render(&mut sprites);
        assert!(buf.content().iter().any(|c| c.symbol() == "▀" || c.symbol() == "▄"));
        assert_eq!(sprites.placements().len(), 1);
    }
}
