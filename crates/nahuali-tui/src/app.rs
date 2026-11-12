//! The interactive loop: crossterm events in, synchronized frames out.

use std::io::{self, Stdout, Write};
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableFocusChange, EnableFocusChange, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::style::Stylize;
use crossterm::terminal::{
    self, BeginSynchronizedUpdate, EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen,
    SetTitle,
};
use crossterm::{cursor, execute, queue};
use nahuali_view::{Effect, MascotState, Screen, UiState, ViewSource, build, keymap};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui_image::picker::{Picker, ProtocolType};

use crate::anim::{Clock, FrameScheduler, Motion, Reason};
use crate::chrome::draw_screen;
use crate::mascot::atlas::Atlas;
use crate::mascot::direct::{Direct, FALLBACK_CELL};
use crate::mascot::{Backend, Sprites};
use crate::theme::{ColorMode, Theme};

/// How long the axolotl keeps "thinking" after the last keystroke.
const SEARCH_SETTLE: Duration = Duration::from_millis(700);

/// Run options.
#[derive(Clone, Debug)]
pub struct Options {
    /// Expose the demo key (`m` cycles the mascot).
    pub demo: bool,
    pub motion: Motion,
    /// Skip the image-protocol query and draw half blocks.
    pub force_halfblocks: bool,
    /// Redraw ceiling.
    pub fps: u32,
    pub color: ColorMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            demo: false,
            motion: Motion::from_env(),
            force_halfblocks: std::env::var_os("NAHUALI_TUI_FORCE_HALF_BLOCKS").is_some(),
            fps: 30,
            color: ColorMode::detect(),
        }
    }
}

/// What a key did, as far as the loop is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Redraw,
    Quit,
}

/// UI state, current screen and sprites. Terminal-free, so tests drive it
/// through [`App::key`] and [`App::render`].
pub struct App<'a> {
    src: &'a dyn ViewSource,
    pub ui: UiState,
    pub screen: Screen,
    pub help: bool,
    pub sprites: Sprites,
    pub theme: Theme,
    search_settle: Option<Instant>,
    /// The questions answered in this run, for the farewell line.
    pub answered: Vec<String>,
}

impl<'a> App<'a> {
    pub fn new(src: &'a dyn ViewSource, ui: UiState, sprites: Sprites, theme: Theme) -> Self {
        let screen = build(src, &ui);
        Self { src, ui, screen, help: false, sprites, theme, search_settle: None, answered: Vec::new() }
    }

    pub fn rebuild(&mut self) {
        self.screen = build(self.src, &self.ui);
    }

    /// The state the screen asks the axolotl to show.
    pub fn mascot_target(&self) -> MascotState {
        self.screen.mascot().unwrap_or_default()
    }

    /// Handle one canonical key (see [`keymap`]); `typed` is the character
    /// for printable keys.
    pub fn key(&mut self, key: &str, typed: Option<char>, now: Instant) -> Flow {
        if self.help {
            self.help = false;
            return Flow::Redraw;
        }
        let notice_before = self.ui.notice.clone();
        let effect = match keymap::resolve(self.ui.view, key, self.ui.demo) {
            _ if self.ui.prompt.is_some() => self.ui.prompt_key(key, typed, self.src),
            Some(action) => self.ui.apply(action, self.src),
            None => match typed {
                Some(c) if self.ui.view.takes_text() => self.ui.type_char(c),
                _ => Effect::None,
            },
        };
        if self.ui.searching {
            self.search_settle = Some(now + SEARCH_SETTLE);
        }
        // Count an answer once: only when this key produced the notice.
        if self.ui.notice != notice_before
            && let Some(text) = self.ui.notice.as_deref().and_then(|n| n.strip_prefix("answered · "))
        {
            self.answered.push(text.to_string());
        }
        match effect {
            Effect::None => Flow::Continue,
            Effect::Redraw => {
                self.rebuild();
                Flow::Redraw
            }
            Effect::Help => {
                self.help = true;
                Flow::Redraw
            }
            Effect::Quit => Flow::Quit,
        }
    }

    /// Advance timers. Returns true when something visible changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let mut changed = false;
        if self.search_settle.is_some_and(|t| now >= t) {
            self.search_settle = None;
            self.ui.searching = false;
            self.rebuild();
            changed = true;
        }
        let target = self.mascot_target();
        changed |= self.sprites.update(target, now);
        changed
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        match (self.search_settle, self.sprites.animator.next_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.sprites.clear_placements();
        let help = self.help.then(|| keymap::help(self.ui.view, self.ui.demo));
        draw_screen(&self.screen, area, buf, &self.theme, &mut self.sprites, help.as_ref());
    }
}

/// Canonical key name for a crossterm key event, plus the typed character.
pub fn key_name(event: &KeyEvent) -> Option<(String, Option<char>)> {
    if event.kind != KeyEventKind::Press && event.kind != KeyEventKind::Repeat {
        return None;
    }
    let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    Some(match event.code {
        KeyCode::Char(c) if ctrl => (format!("ctrl+{}", c.to_ascii_lowercase()), None),
        KeyCode::Char(c) => (c.to_string(), Some(c)),
        KeyCode::Enter => ("enter".into(), None),
        KeyCode::Esc => ("esc".into(), None),
        KeyCode::Tab => ("tab".into(), None),
        KeyCode::BackTab => ("backtab".into(), None),
        KeyCode::Backspace => ("backspace".into(), None),
        KeyCode::Up => ("up".into(), None),
        KeyCode::Down => ("down".into(), None),
        KeyCode::Left => ("left".into(), None),
        KeyCode::Right => ("right".into(), None),
        _ => return None,
    })
}

type Term = Terminal<CrosstermBackend<Stdout>>;

fn enter() -> io::Result<Term> {
    terminal::enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen, EnableFocusChange, cursor::Hide)?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = leave();
        hook(info);
    }));
    Terminal::new(CrosstermBackend::new(out))
}

fn leave() -> io::Result<()> {
    let mut out = io::stdout();
    execute!(out, cursor::Show, DisableFocusChange, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()
}

/// Draw one frame inside a synchronized update (DEC mode 2026), so the
/// terminal shows it whole: no tearing when the axolotl and a list change
/// in the same frame. A directly placed sprite goes in after the cells,
/// inside the same update.
fn draw(terminal: &mut Term, app: &mut App<'_>) -> io::Result<()> {
    queue!(terminal.backend_mut(), BeginSynchronizedUpdate)?;
    let mut screen = Size::default();
    terminal.draw(|frame| {
        let area = frame.area();
        screen = area.as_size();
        app.render(area, frame.buffer_mut());
    })?;
    let sprites = app.sprites.direct_commands(screen);
    terminal.backend_mut().write_all(&sprites)?;
    execute!(terminal.backend_mut(), EndSynchronizedUpdate)?;
    Ok(())
}

/// How to reach the terminal's image support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Graphics {
    /// Ask with `ratatui-image`, which then uses Unicode placeholders.
    Query,
    /// Kitty direct placement, for a terminal without placeholders.
    Direct,
    Halfblocks,
}

/// Decide from the environment alone. The capability query of
/// `ratatui-image` leaves a stdin reader running when a terminal never
/// answers (it then swallows keys and drops raw mode), so it only runs on
/// terminals known to draw images: Kitty, Ghostty, WezTerm and iTerm2. Tern
/// answers as Kitty but draws no Unicode placeholders, so it skips the query
/// and places images directly.
/// `NAHUALI_TUI_GRAPHICS=query|direct|halfblocks` overrides the guess.
pub fn graphics_from_env(var: impl Fn(&str) -> Option<String>) -> Graphics {
    match var("NAHUALI_TUI_GRAPHICS").as_deref() {
        Some("query") => return Graphics::Query,
        Some("direct") => return Graphics::Direct,
        Some("halfblocks") => return Graphics::Halfblocks,
        _ => {}
    }
    let program = var("TERM_PROGRAM").unwrap_or_default().to_ascii_lowercase();
    let known_program = matches!(program.as_str(), "ghostty" | "wezterm" | "iterm.app" | "kitty");
    let kitty = var("TERM").is_some_and(|t| t == "xterm-kitty") || var("KITTY_WINDOW_ID").is_some();
    let ghostty = var("GHOSTTY_RESOURCES_DIR").is_some();
    if var("TMUX").is_some() {
        Graphics::Halfblocks
    } else if program == "tern" {
        Graphics::Direct
    } else if known_program || kitty || ghostty {
        Graphics::Query
    } else {
        Graphics::Halfblocks
    }
}

/// What the terminal gets and why, for `nahuali doctor`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphicsReport {
    /// `kitty placeholders`, `tern direct`, `iterm2` or `half blocks`.
    pub mode: &'static str,
    /// In a sentence.
    pub why: String,
}

/// [`graphics_from_env`] in words. Only the environment is read: the
/// capability query needs a live terminal, and the TUI makes it itself.
pub fn graphics_report(var: impl Fn(&str) -> Option<String>) -> GraphicsReport {
    let program = var("TERM_PROGRAM").unwrap_or_default();
    let (mode, why) = match graphics_from_env(&var) {
        Graphics::Direct => ("tern direct", "frames placed with Kitty direct placement".to_string()),
        Graphics::Query if program.eq_ignore_ascii_case("iterm.app") => {
            ("iterm2", "inline images, capped at 4 fps because iTerm2 redraws whole frames".to_string())
        }
        Graphics::Query => {
            ("kitty placeholders", "frames sent once, then drawn with Unicode placeholders".to_string())
        }
        Graphics::Halfblocks if var("TMUX").is_some() => (
            "half blocks",
            "tmux hides image protocols, so the axolotl is drawn in coloured cells".to_string(),
        ),
        Graphics::Halfblocks => {
            let shown = if program.is_empty() { "this terminal" } else { program.as_str() };
            (
                "half blocks",
                format!("{shown} is not known to draw images; Kitty, Ghostty, WezTerm, iTerm2 and Tern do"),
            )
        }
    };
    GraphicsReport { mode, why }
}

/// The cell size in pixels, as the terminal reports it without being asked.
fn cell_size() -> Size {
    terminal::window_size()
        .ok()
        .filter(|w| w.columns > 0 && w.rows > 0 && w.width >= w.columns && w.height >= w.rows)
        .map_or(FALLBACK_CELL, |w| Size::new(w.width / w.columns, w.height / w.rows))
}

fn backend(options: &Options) -> Backend {
    if options.force_halfblocks {
        return Backend::Halfblocks;
    }
    match graphics_from_env(|k| std::env::var(k).ok()) {
        Graphics::Halfblocks => return Backend::Halfblocks,
        Graphics::Direct => return Backend::Direct(Direct::new(cell_size())),
        Graphics::Query => {}
    }
    match Picker::from_query_stdio() {
        Ok(mut picker) => {
            picker.set_background_color(None::<image::Rgba<u8>>);
            match picker.protocol_type() {
                ProtocolType::Kitty | ProtocolType::Iterm2 => Backend::Raster(Box::new(picker)),
                // Sixel redraws poorly under frequent updates; half blocks
                // look better than a flickering sprite.
                ProtocolType::Sixel | ProtocolType::Halfblocks => Backend::Halfblocks,
            }
        }
        Err(_) => Backend::Halfblocks,
    }
}

/// Run the TUI until the user quits.
pub fn run(src: &dyn ViewSource, options: Options) -> io::Result<()> {
    let mut terminal = enter()?;
    let result = run_inner(&mut terminal, src, &options);
    leave()?;
    let answered = result?;
    farewell(&answered);
    Ok(())
}

fn run_inner(terminal: &mut Term, src: &dyn ViewSource, options: &Options) -> io::Result<Vec<String>> {
    let theme = Theme::new(options.color);
    let now = Instant::now();
    let clock = Clock::new(now);
    let sprites = Sprites::new(
        Atlas::discover(),
        backend(options),
        options.color,
        theme.bg(),
        options.motion,
        clock,
        now,
    );
    let mut app = App::new(src, UiState::new(options.demo), sprites, theme);
    let mut scheduler = FrameScheduler::new(options.fps);
    scheduler.request(Reason::Input);
    let mut title = String::new();
    loop {
        let now = Instant::now();
        if app.tick(now) {
            scheduler.request(Reason::Animation);
        }
        if scheduler.should_draw(now) {
            if app.screen.title != title {
                title.clone_from(&app.screen.title);
                execute!(terminal.backend_mut(), SetTitle(&title))?;
            }
            draw(terminal, &mut app)?;
            scheduler.drawn(now);
        }
        let wake = match (scheduler.due(Instant::now()), app.next_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let timeout =
            wake.map(|w| w.saturating_duration_since(Instant::now())).unwrap_or(Duration::from_secs(30));
        if !event::poll(timeout)? {
            continue;
        }
        // Input first: drain everything pending before the next frame.
        loop {
            let now = Instant::now();
            match event::read()? {
                Event::Key(key) => {
                    if let Some((name, typed)) = key_name(&key) {
                        match app.key(&name, typed, now) {
                            Flow::Quit => {
                                let sprites = app.sprites.release();
                                terminal.backend_mut().write_all(&sprites)?;
                                terminal.backend_mut().flush()?;
                                return Ok(app.answered);
                            }
                            Flow::Redraw => scheduler.request(Reason::Input),
                            Flow::Continue => {}
                        }
                    }
                }
                Event::Resize(..) => {
                    app.sprites.set_cell(cell_size());
                    scheduler.request(Reason::Input);
                }
                Event::FocusLost => app.sprites.set_focus(false, now),
                Event::FocusGained => {
                    app.sprites.set_focus(true, now);
                    scheduler.request(Reason::Animation);
                }
                _ => {}
            }
            if !event::poll(Duration::ZERO)? {
                break;
            }
        }
    }
}

/// Leave a trace in the scrollback: the face and what changed.
fn farewell(answered: &[String]) {
    let mut out = io::stdout();
    let mark = format!("{}{}{}", "≋".red().bold(), "(•ᴗ•)", "≋".red().bold());
    let line = match answered.len() {
        0 => "nahuali · see you".to_string(),
        1 => format!("nahuali · answered: {}", answered[0]),
        n => format!("nahuali · answered {n} questions"),
    };
    let _ = writeln!(out, "{mark} {}", line.dark_grey());
}

#[cfg(test)]
mod tests {
    use super::*;
    use nahuali_view::View;
    use nahuali_view::mock::MockSource;

    fn app(src: &MockSource) -> App<'_> {
        let theme = Theme::new(ColorMode::TrueColor);
        App::new(src, UiState::new(true), Sprites::for_tests(ColorMode::TrueColor, theme.bg()), theme)
    }

    fn type_text(app: &mut App<'_>, text: &str, now: Instant) {
        for c in text.chars() {
            app.key(&c.to_string(), Some(c), now);
        }
    }

    #[test]
    fn an_answer_is_typed_recorded_and_counted_once() {
        let src = MockSource::new();
        let mut app = app(&src);
        let now = Instant::now();
        assert_eq!(app.key("a", Some('a'), now), Flow::Redraw);
        assert!(app.ui.prompt.is_some(), "the first row is a question");
        // While the prompt is open, letters that are keys elsewhere are text.
        type_text(&mut app, "behind a flag, q for quality", now);
        assert_eq!(app.key("enter", None, now), Flow::Redraw);
        assert_eq!(
            src.answers(),
            vec![("ask:importer-flag".to_string(), "behind a flag, q for quality".to_string())]
        );
        assert_eq!(app.answered.len(), 1);
        app.key("j", Some('j'), now);
        app.key("k", Some('k'), now);
        assert_eq!(app.answered.len(), 1);
        assert_eq!(src.board().asks.len(), 1, "the answered question left the board");
    }

    #[test]
    fn an_empty_answer_is_not_recorded_and_esc_cancels() {
        let src = MockSource::new();
        let mut app = app(&src);
        let now = Instant::now();
        app.key("a", Some('a'), now);
        app.key("enter", None, now);
        assert!(src.answers().is_empty());
        assert!(app.ui.prompt.is_some(), "the prompt stays open for the words");
        app.key("esc", None, now);
        assert!(app.ui.prompt.is_none());
        assert!(src.answers().is_empty());
    }

    #[test]
    fn the_axolotl_follows_what_is_owed() {
        let src = MockSource::new();
        let mut app = app(&src);
        let now = Instant::now();
        assert_eq!(app.mascot_target(), MascotState::Worried, "a question has waited two days");
        app.key("a", Some('a'), now);
        type_text(&mut app, "flag", now);
        app.key("enter", None, now);
        assert_eq!(app.mascot_target(), MascotState::Listening, "one question still waits");
        app.key("a", Some('a'), now);
        type_text(&mut app, "the second", now);
        app.key("enter", None, now);
        assert_eq!(app.mascot_target(), MascotState::Happy, "nothing waits any more");
        assert_eq!(app.ui.view, View::Board);
    }

    #[test]
    fn help_over_a_directly_placed_sprite_removes_it_until_it_closes() {
        let src = MockSource::new();
        let theme = Theme::new(ColorMode::TrueColor);
        let sprites = Sprites::direct_for_tests(ColorMode::TrueColor, theme.bg());
        let mut app = App::new(&src, UiState::new(true), sprites, theme);
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let now = Instant::now();
        app.render(area, &mut buf);
        assert_eq!(app.sprites.placements().len(), 1);
        app.key("?", Some('?'), now);
        app.render(area, &mut buf);
        assert!(app.sprites.placements().is_empty(), "the card would sit under the image");
        app.key("x", Some('x'), now);
        app.render(area, &mut buf);
        assert_eq!(app.sprites.placements().len(), 1);
    }

    #[test]
    fn help_opens_and_any_key_closes_it() {
        let src = MockSource::new();
        let mut app = app(&src);
        let now = Instant::now();
        assert_eq!(app.key("?", Some('?'), now), Flow::Redraw);
        assert!(app.help);
        assert_eq!(app.key("q", Some('q'), now), Flow::Redraw);
        assert!(!app.help);
        assert_eq!(app.key("q", Some('q'), now), Flow::Quit);
    }

    #[test]
    fn only_known_image_terminals_are_queried() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
        };
        assert_eq!(graphics_from_env(env(&[("TERM_PROGRAM", "Apple_Terminal")])), Graphics::Halfblocks);
        assert_eq!(graphics_from_env(env(&[("TERM", "xterm-256color")])), Graphics::Halfblocks);
        assert_eq!(graphics_from_env(env(&[("TERM", "xterm-kitty")])), Graphics::Query);
        assert_eq!(graphics_from_env(env(&[("TERM_PROGRAM", "ghostty")])), Graphics::Query);
        assert_eq!(graphics_from_env(env(&[("TERM_PROGRAM", "iTerm.app")])), Graphics::Query);
        assert_eq!(
            graphics_from_env(env(&[("TERM_PROGRAM", "tern"), ("TERM", "xterm-kitty")])),
            Graphics::Direct,
            "Tern never gets the query, even when it looks like Kitty"
        );
        assert_eq!(
            graphics_from_env(env(&[("TERM_PROGRAM", "tern"), ("TMUX", "/tmp/x")])),
            Graphics::Halfblocks
        );
        assert_eq!(
            graphics_from_env(env(&[("TERM", "dumb"), ("NAHUALI_TUI_GRAPHICS", "direct")])),
            Graphics::Direct
        );
        assert_eq!(
            graphics_from_env(env(&[("TERM_PROGRAM", "tern"), ("NAHUALI_TUI_GRAPHICS", "query")])),
            Graphics::Query
        );
        assert_eq!(
            graphics_from_env(env(&[("TERM_PROGRAM", "ghostty"), ("TMUX", "/tmp/x")])),
            Graphics::Halfblocks
        );
        assert_eq!(
            graphics_from_env(env(&[("TERM", "dumb"), ("NAHUALI_TUI_GRAPHICS", "query")])),
            Graphics::Query
        );
        assert_eq!(
            graphics_from_env(env(&[("TERM", "xterm-kitty"), ("NAHUALI_TUI_GRAPHICS", "halfblocks")])),
            Graphics::Halfblocks
        );
    }

    #[test]
    fn key_names_are_canonical() {
        use crossterm::event::{KeyEventState, KeyModifiers};
        let event = |code, modifiers| KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        assert_eq!(
            key_name(&event(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(("ctrl+c".into(), None))
        );
        assert_eq!(key_name(&event(KeyCode::Char('J'), KeyModifiers::SHIFT)), Some(("J".into(), Some('J'))));
        assert_eq!(key_name(&event(KeyCode::BackTab, KeyModifiers::SHIFT)).unwrap().0, "backtab");
        let release = KeyEvent { kind: KeyEventKind::Release, ..event(KeyCode::Enter, KeyModifiers::NONE) };
        assert_eq!(key_name(&release), None);
    }
}
