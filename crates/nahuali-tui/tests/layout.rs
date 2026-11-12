//! Layout rules: the narrow fallback, stacking, and the footer's drop order.

use nahuali_tui::App;
use nahuali_tui::chrome::footer_fit;
use nahuali_tui::mascot::Sprites;
use nahuali_tui::theme::{ColorMode, Theme};
use nahuali_view::mock::MockSource;
use nahuali_view::{UiState, View, build};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn draw_ui(src: &MockSource, ui: UiState, w: u16, h: u16) -> (Buffer, Vec<Rect>) {
    let theme = Theme::new(ColorMode::TrueColor);
    let mut app = App::new(src, ui, Sprites::for_tests(ColorMode::TrueColor, theme.bg()), theme);
    let target = app.mascot_target();
    app.sprites.force(target, 0);
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    app.render(area, &mut buf);
    let rects = app.sprites.placements().iter().map(|p| p.rect).collect();
    (buf, rects)
}

fn draw(src: &MockSource, view: View, w: u16, h: u16) -> (Buffer, Vec<Rect>) {
    let mut ui = UiState::new(false);
    ui.view = view;
    draw_ui(src, ui, w, h)
}

fn row(buf: &Buffer, y: u16) -> String {
    (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
}

fn find(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
    (0..buf.area.height).find_map(|y| {
        let line = row(buf, y);
        line.find(needle).map(|byte| {
            let x = unicode_width::UnicodeWidthStr::width(&line[..byte]) as u16;
            (x, y)
        })
    })
}

#[test]
fn a_wide_board_keeps_the_axolotl_in_a_sidebar() {
    let src = MockSource::new();
    let (buf, sprites) = draw(&src, View::Board, 120, 36);
    let sprite = sprites[0];
    assert!(sprite.x < 32 && sprite.width >= 20, "{sprite:?}");
    assert!(sprite.height >= 10, "a real sprite, not a stamp: {sprite:?}");
    let (waiting_x, waiting_y) = find(&buf, "Waiting on you").unwrap();
    assert!(waiting_x > 32, "cards sit right of the sidebar");
    let (_, motion_y) = find(&buf, "In motion").unwrap();
    let (_, learned_y) = find(&buf, "Learned this week").unwrap();
    assert!(waiting_y < motion_y && motion_y < learned_y, "what is owed comes first");
    // Tab bar: active tab framed on its own row above the labels.
    assert!(row(&buf, 0).contains("╭"));
    assert!(row(&buf, 1).contains("Lessons"));
}

#[test]
fn a_narrow_board_turns_the_sidebar_into_a_band() {
    let src = MockSource::new();
    let (buf, sprites) = draw(&src, View::Board, 80, 24);
    let sprite = sprites[0];
    assert!(sprite.y <= 3 && sprite.height <= 6, "band sprite at the top: {sprite:?}");
    let (caption_x, caption_y) = find(&buf, "worried").unwrap();
    assert!(caption_x > sprite.right() && caption_y == sprite.y, "caption beside it");
    let (_, waiting_y) = find(&buf, "Waiting on you").unwrap();
    assert!(waiting_y >= sprite.bottom(), "cards below the band");
    // What is owed always fits; what does not fit is dropped, not mangled.
    assert!(find(&buf, "Ship the importer behind a flag").is_some());
    assert!(find(&buf, "Which of the two logos").is_some());
}

#[test]
fn an_open_item_shows_its_four_marks_at_any_size() {
    let src = MockSource::new();
    for (w, h) in [(120, 36), (80, 24)] {
        let mut ui = UiState::new(false);
        ui.detail = true;
        let (buf, _) = draw_ui(&src, ui, w, h);
        for mark in ["who", "how sure", "as of", "reach"] {
            assert!(find(&buf, mark).is_some(), "{mark} at {w}x{h}");
        }
        assert!(find(&buf, "observed by claude-code").is_some(), "{w}x{h}");
    }
}

#[test]
fn an_empty_memory_says_so() {
    let src = MockSource::empty();
    let (buf, _) = draw(&src, View::Board, 120, 36);
    assert!(find(&buf, "Nothing waits on you").is_some());
    assert!(find(&buf, "Nothing in motion").is_some());
    let (buf, _) = draw(&src, View::Lessons, 120, 36);
    assert!(find(&buf, "Nothing learned yet").is_some());
}

#[test]
fn tiny_windows_say_so_instead_of_panicking() {
    let src = MockSource::new();
    for view in View::ALL {
        for (w, h) in [(10, 4), (19, 7), (40, 12), (60, 20), (300, 80)] {
            let (buf, _) = draw(&src, view, w, h);
            assert_eq!(buf.area.width, w);
        }
    }
    let (buf, _) = draw(&src, View::Board, 19, 7);
    assert!(row(&buf, 0).starts_with("nahuali"));
}

#[test]
fn footer_drops_globals_first_and_keeps_help() {
    let src = MockSource::new();
    let theme = Theme::new(ColorMode::TrueColor);
    let screen = build(&src, &UiState::new(false));
    let label = |i: usize| screen.keys[i].label.as_str();
    let (segs, hints) = footer_fit(&screen, 200, &theme);
    assert_eq!(segs.len(), screen.status.len());
    assert_eq!(hints.len(), screen.keys.len());
    let (segs, hints) = footer_fit(&screen, 80, &theme);
    let kept: Vec<&str> = hints.iter().map(|i| label(*i)).collect();
    assert!(kept.contains(&"help"), "{kept:?}");
    assert!(kept.contains(&"answer"), "the view's own keys outlast globals: {kept:?}");
    assert!(segs.contains(&0), "the pinned mascot segment stays");
    let (segs, hints) = footer_fit(&screen, 30, &theme);
    assert_eq!(segs, vec![0]);
    assert!(hints.is_empty());
}
