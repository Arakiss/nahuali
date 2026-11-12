//! Golden snapshots of every screen at 120×36 and 80×24, drawn through a
//! ratatui `TestBackend` exactly as the TUI draws them (half-block mascot).

use nahuali_tui::App;
use nahuali_tui::mascot::Sprites;
use nahuali_tui::theme::{ColorMode, Theme};
use nahuali_view::mock::MockSource;
use nahuali_view::{UiState, View};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

fn text(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf[(x, y)].symbol());
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

fn render(src: MockSource, view: View, w: u16, h: u16, setup: impl Fn(&MockSource, &mut UiState)) -> String {
    let mut ui = UiState::new(false);
    ui.view = view;
    setup(&src, &mut ui);
    let theme = Theme::new(ColorMode::TrueColor);
    let sprites = Sprites::for_tests(ColorMode::TrueColor, theme.bg());
    let mut app = App::new(&src, ui, sprites, theme);
    let target = app.mascot_target();
    app.sprites.force(target, 0);
    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
    terminal
        .draw(|frame| {
            let area = frame.area();
            app.render(area, frame.buffer_mut());
        })
        .unwrap();
    text(terminal.backend().buffer())
}

macro_rules! snap {
    ($name:ident, $view:expr) => {
        snap!($name, MockSource::new(), $view, |_, _| {});
    };
    ($name:ident, $src:expr, $view:expr, $setup:expr) => {
        #[test]
        fn $name() {
            insta::assert_snapshot!(
                concat!(stringify!($name), "_120x36"),
                render($src, $view, 120, 36, $setup)
            );
            insta::assert_snapshot!(
                concat!(stringify!($name), "_80x24"),
                render($src, $view, 80, 24, $setup)
            );
        }
    };
}

snap!(board, View::Board);
snap!(board_empty, MockSource::empty(), View::Board, |_, _| {});
snap!(board_question, MockSource::new(), View::Board, |_: &MockSource, ui: &mut UiState| {
    ui.detail = true
});
snap!(board_work, MockSource::new(), View::Board, |_: &MockSource, ui: &mut UiState| {
    ui.select(View::Board, 2);
    ui.detail = true;
});
snap!(board_answered, MockSource::new(), View::Board, |src: &MockSource, _: &mut UiState| {
    use nahuali_view::ViewSource;
    src.answer("ask:importer-flag", "behind a flag").unwrap();
    src.answer("ask:cover-logo", "the second").unwrap();
});
snap!(lessons, View::Lessons);
snap!(lesson_open, MockSource::new(), View::Lessons, |_: &MockSource, ui: &mut UiState| { ui.detail = true });
