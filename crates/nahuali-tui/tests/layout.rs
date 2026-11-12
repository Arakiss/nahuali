//! Layout rules: the narrow fallback, stacking, and the footer's drop order.

use nahuali_tui::App;
use nahuali_tui::chrome::footer_fit;
use nahuali_tui::mascot::Sprites;
use nahuali_tui::theme::{ColorMode, Theme};
use nahuali_view::mock::MockSource;
use nahuali_view::{UiState, View, build};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

fn draw(src: &MockSource, view: View, w: u16, h: u16) -> (Buffer, Vec<Rect>) {
    let mut ui = UiState::new(false);
    ui.view = view;
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
fn wide_today_keeps_the_axolotl_in_a_sidebar() {
    let src = MockSource::new();
    let (buf, sprites) = draw(&src, View::Today, 120, 36);
    let sprite = sprites[0];
    assert!(sprite.x < 32 && sprite.width >= 20, "{sprite:?}");
    assert!(sprite.height >= 10, "a real sprite, not a stamp: {sprite:?}");
    let (needs_x, _) = find(&buf, "Needs attention").unwrap();
    assert!(needs_x > 32, "cards sit right of the sidebar");
    // Tab bar: active tab framed on its own row above the labels.
    assert!(row(&buf, 0).contains("╭"));
    assert!(row(&buf, 1).contains("Journal"));
}

#[test]
fn narrow_today_turns_the_sidebar_into_a_band() {
    let src = MockSource::new();
    let (buf, sprites) = draw(&src, View::Today, 80, 24);
    let sprite = sprites[0];
    assert!(sprite.y <= 3 && sprite.height <= 6, "band sprite at the top: {sprite:?}");
    let (caption_x, caption_y) = find(&buf, "worried").unwrap();
    assert!(caption_x > sprite.right() && caption_y == sprite.y, "caption beside it");
    let (_, needs_y) = find(&buf, "Needs attention").unwrap();
    assert!(needs_y >= sprite.bottom(), "cards below the band");
    // Every need fits; the least important card is dropped, not mangled.
    assert!(find(&buf, "Draw the remaining axolotl states").is_some());
    assert!(find(&buf, "Open threads").is_none());
}

#[test]
fn narrow_intentions_stack_their_columns() {
    let src = MockSource::new();
    let (buf, _) = draw(&src, View::Intentions, 80, 24);
    let overdue = find(&buf, "Overdue (2)").unwrap();
    let open = find(&buf, "Open (5)").unwrap();
    let closed = find(&buf, "Closed this week (5)").unwrap();
    assert!(overdue.1 < open.1 && open.1 < closed.1, "stacked top to bottom");
    let (wide, _) = draw(&src, View::Intentions, 120, 36);
    let a = find(&wide, "Overdue (2)").unwrap();
    let b = find(&wide, "Open (5)").unwrap();
    assert_eq!(a.1, b.1, "side by side when roomy");
}

#[test]
fn health_alarm_puts_the_axolotl_on_top_at_any_size() {
    let src = MockSource::new();
    src.set_chain_broken(true);
    for (w, h) in [(120, 36), (80, 24)] {
        let (buf, sprites) = draw(&src, View::Health, w, h);
        assert_eq!(sprites.len(), 1, "{w}x{h}");
        let (_, caption_y) = find(&buf, "alarmed").unwrap();
        let (_, integrity_y) = find(&buf, "Integrity").unwrap();
        assert!(caption_y < integrity_y);
        assert!(find(&buf, "event chain breaks at seq 48197").is_some());
    }
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
    let (buf, _) = draw(&src, View::Today, 19, 7);
    assert!(row(&buf, 0).starts_with("nahuali"));
}

#[test]
fn footer_drops_globals_first_and_keeps_help() {
    let src = MockSource::new();
    let theme = Theme::new(ColorMode::TrueColor);
    let mut ui = UiState::new(false);
    ui.view = View::Intentions;
    let screen = build(&src, &ui);
    let label = |i: usize| screen.keys[i].label.as_str();
    let (segs, hints) = footer_fit(&screen, 200, &theme);
    assert_eq!(segs.len(), screen.status.len());
    assert_eq!(hints.len(), screen.keys.len());
    let (segs, hints) = footer_fit(&screen, 80, &theme);
    let kept: Vec<&str> = hints.iter().map(|i| label(*i)).collect();
    assert!(kept.contains(&"help"), "{kept:?}");
    assert!(kept.contains(&"column"), "the view's own keys outlast globals: {kept:?}");
    assert!(!kept.contains(&"views"), "{kept:?}");
    assert!(segs.contains(&0), "the pinned mascot segment stays");
    let (segs, hints) = footer_fit(&screen, 30, &theme);
    assert_eq!(segs, vec![0]);
    assert!(hints.is_empty());
}
