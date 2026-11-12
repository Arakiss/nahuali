//! Golden snapshots of every screen at 120×36 and 80×24, drawn through a
//! ratatui `TestBackend` exactly as the TUI draws them (half-block mascot).

use nahuali_tui::App;
use nahuali_tui::mascot::Sprites;
use nahuali_tui::theme::{ColorMode, Theme};
use nahuali_view::mock::MockSource;
use nahuali_view::source::{Extensions, Principal, PrincipalKind};
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

fn render(view: View, w: u16, h: u16, setup: impl Fn(&MockSource, &mut UiState)) -> String {
    render_from(MockSource::new(), view, w, h, setup)
}

fn render_from(
    src: MockSource,
    view: View,
    w: u16,
    h: u16,
    setup: impl Fn(&MockSource, &mut UiState),
) -> String {
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
        snap!($name, $view, |_, _| {});
    };
    ($name:ident, $view:expr, $setup:expr) => {
        #[test]
        fn $name() {
            insta::assert_snapshot!(concat!(stringify!($name), "_120x36"), render($view, 120, 36, $setup));
            insta::assert_snapshot!(concat!(stringify!($name), "_80x24"), render($view, 80, 24, $setup));
        }
    };
}

snap!(today, View::Today);
snap!(journal, View::Journal);
snap!(intentions, View::Intentions);
snap!(health, View::Health);
snap!(health_alarm, View::Health, |src: &MockSource, _: &mut UiState| { src.set_chain_broken(true) });
snap!(search_empty, View::Search);
snap!(search, View::Search, |_: &MockSource, ui: &mut UiState| {
    ui.query = "why did we keep the store local".into();
    ui.searching = true;
});
snap!(proposals, View::Proposals);
snap!(proposals_diff, View::Proposals, |_: &MockSource, ui: &mut UiState| { ui.detail = true });
snap!(search_record, View::Search, |_: &MockSource, ui: &mut UiState| {
    ui.query = "why did we keep the store local".into();
    ui.detail = true;
});

/// The core alone: five tabs, no loops card, no candidate or update rows.
macro_rules! snap_from {
    ($name:ident, $src:expr, $view:expr) => {
        #[test]
        fn $name() {
            insta::assert_snapshot!(
                concat!(stringify!($name), "_120x36"),
                render_from($src, $view, 120, 36, |_, _| {})
            );
            insta::assert_snapshot!(
                concat!(stringify!($name), "_80x24"),
                render_from($src, $view, 80, 24, |_, _| {})
            );
        }
    };
}

snap_from!(core_today, MockSource::new().with_extensions(Extensions::NONE), View::Today);
snap_from!(core_health, MockSource::new().with_extensions(Extensions::NONE), View::Health);
snap_from!(
    team_today,
    MockSource::new().with_principal(Principal::new("infra", PrincipalKind::Team)),
    View::Today
);
