//! The screens against the promises of `docs/product.md`, on the mock memory.

use nahuali_view::mock::MockSource;
use nahuali_view::screens::board_mood;
use nahuali_view::source::{Sure, ViewSource};
use nahuali_view::state::{Picked, picked};
use nahuali_view::{Action, Effect, Item, MascotState, Node, Screen, UiState, View, build};

fn ui(view: View) -> UiState {
    let mut ui = UiState::new(false);
    ui.view = view;
    ui
}

fn cards(node: &Node, out: &mut Vec<String>) {
    match node {
        Node::Card { title, body, .. } => {
            out.push(title.clone());
            cards(body, out);
        }
        Node::Col(children) | Node::Row(children) => children.iter().for_each(|c| cards(c, out)),
        _ => {}
    }
}

fn card_titles(screen: &Screen) -> Vec<String> {
    let mut out = Vec::new();
    cards(&screen.body, &mut out);
    out
}

fn kv(node: &Node, out: &mut Vec<(String, String)>) {
    match node {
        Node::Kv(pairs) => out.extend(pairs.iter().cloned()),
        Node::Card { body, .. } => kv(body, out),
        Node::Col(children) | Node::Row(children) => children.iter().for_each(|c| kv(c, out)),
        _ => {}
    }
}

fn pairs(screen: &Screen) -> Vec<(String, String)> {
    let mut out = Vec::new();
    kv(&screen.body, &mut out);
    out
}

fn item_text(item: &Item) -> String {
    item.text.iter().map(|s| s.text.as_str()).collect()
}

fn type_answer(ui: &mut UiState, src: &MockSource, text: &str) {
    for c in text.chars() {
        ui.prompt_key(&c.to_string(), Some(c), src);
    }
}

// --- State before history ---------------------------------------------------

#[test]
fn the_first_screen_says_what_is_owed_before_anything_else() {
    let src = MockSource::new();
    let screen = build(&src, &UiState::new(false));
    assert_eq!(screen.active_tab, 0);
    assert_eq!(screen.tabs, ["Board", "Lessons"]);
    assert_eq!(screen.who, "ada");
    let titles = card_titles(&screen);
    let at = |needle: &str| titles.iter().position(|t| t.starts_with(needle)).unwrap();
    assert!(at("Waiting on you · 2") < at("In motion · 3"));
    assert!(at("In motion · 3") < at("Learned this week · 2"));
}

#[test]
fn a_work_row_says_where_it_stands_in_one_line() {
    let src = MockSource::new();
    let board = src.board();
    let Some(Picked::Work(importer)) = picked(&board, 2) else { panic!("row 2 is the importer") };
    assert_eq!(importer.line, "tests green, review left");
    let mut open = ui(View::Board);
    open.select(View::Board, 2);
    open.detail = true;
    let screen = build(&src, &open);
    assert!(card_titles(&screen).iter().any(|t| t == "Work · atlas"));
    let rows = pairs(&screen);
    assert!(rows.contains(&("status".into(), "moving".into())));
    assert!(rows.contains(&("now".into(), "tests green, review left".into())));
}

// --- Nothing owed is lost ---------------------------------------------------

#[test]
fn an_ask_stays_in_view_until_it_is_answered() {
    let src = MockSource::new();
    let mut ui = UiState::new(false);
    assert_eq!(src.board().asks.len(), 2);
    assert_eq!(ui.apply(Action::Answer, &src), Effect::Redraw);
    let prompt = ui.prompt.clone().expect("the first row is a question");
    assert_eq!(prompt.ask, "ask:importer-flag");
    // Enter with no words records nothing and keeps the prompt.
    ui.prompt_key("enter", None, &src);
    assert!(src.answers().is_empty() && ui.prompt.is_some());
    type_answer(&mut ui, &src, "behind a flag");
    ui.prompt_key("enter", None, &src);
    assert_eq!(src.answers(), vec![("ask:importer-flag".to_string(), "behind a flag".to_string())]);
    assert!(ui.prompt.is_none());
    let board = src.board();
    assert_eq!(board.asks.len(), 1, "it left the board");
    assert_eq!(board.asks[0].id, "ask:cover-logo", "and the other one did not");
}

#[test]
fn only_a_question_can_be_answered() {
    let src = MockSource::new();
    let mut ui = UiState::new(false);
    ui.select(View::Board, 3);
    ui.apply(Action::Answer, &src);
    assert!(ui.prompt.is_none());
    assert_eq!(ui.notice.as_deref(), Some("select a question to answer it"));
    let empty = MockSource::empty();
    let mut ui = UiState::new(false);
    ui.apply(Action::Answer, &empty);
    assert_eq!(ui.notice.as_deref(), Some("nothing waits on you"));
}

#[test]
fn an_answer_moves_the_work_it_blocked() {
    let src = MockSource::new();
    let cover = |src: &MockSource| src.board().works.into_iter().find(|w| w.id == "work:cover").unwrap();
    assert_eq!(cover(&src).status.word(), "waiting on you");
    src.answer("ask:cover-logo", "the second").unwrap();
    assert_eq!(cover(&src).status.word(), "moving");
    assert!(src.answer("ask:cover-logo", "again").is_err(), "an ask is answered once");
}

// --- Provenance in sight ----------------------------------------------------

#[test]
fn every_open_item_shows_its_four_marks() {
    let src = MockSource::new();
    let rows = src.board().asks.len() + src.board().works.len();
    for row in 0..rows {
        let mut open = ui(View::Board);
        open.select(View::Board, row);
        open.detail = true;
        let keys: Vec<String> = pairs(&build(&src, &open)).into_iter().map(|(k, _)| k).collect();
        for mark in ["who", "how sure", "as of", "reach"] {
            assert!(keys.iter().any(|k| k == mark), "row {row} hides `{mark}`");
        }
    }
    for row in 0..src.lessons().len() {
        let mut open = ui(View::Lessons);
        open.select(View::Lessons, row);
        open.detail = true;
        let keys: Vec<String> = pairs(&build(&src, &open)).into_iter().map(|(k, _)| k).collect();
        for mark in ["who", "how sure", "as of", "reach", "when"] {
            assert!(keys.iter().any(|k| k == mark), "lesson {row} hides `{mark}`");
        }
    }
}

#[test]
fn the_marks_tell_verified_from_claimed_and_whose_word_it_is() {
    let src = MockSource::new();
    let mut open = ui(View::Board);
    open.select(View::Board, 2);
    open.detail = true;
    let rows = pairs(&build(&src, &open));
    assert!(rows.contains(&("who".into(), "observed by claude-code".into())));
    assert!(rows.contains(&("how sure".into(), "verified · cargo test, 212 passed".into())));
    assert!(rows.contains(&("as of".into(), "12m ago · commit 4c1e9a2".into())));
    assert!(rows.contains(&("reach".into(), "atlas".into())));
    // Something an agent passes on from someone else is never shown as verified.
    let printer = src.lessons().into_iter().find(|l| l.id == "lesson:logo-sizes").unwrap();
    assert_eq!(printer.marks.who.line(), "reported by the print shop");
    assert_eq!(printer.marks.sure, Sure::Claimed);
}

#[test]
fn the_status_line_counts_what_was_only_claimed() {
    let src = MockSource::new();
    let screen = build(&src, &UiState::new(false));
    assert!(screen.status.iter().any(|s| s.text == "2 only claimed"), "{:?}", screen.status);
    let side = pairs(&screen);
    assert!(side.contains(&("verified".into(), "5".into())));
    assert!(side.contains(&("claimed".into(), "2".into())));
    let empty = build(&MockSource::empty(), &UiState::new(false));
    assert!(empty.status.iter().any(|s| s.text == "nothing held yet"));
}

// --- The lesson and its moment ----------------------------------------------

#[test]
fn a_lesson_is_a_rule_and_the_moment_it_applies() {
    let src = MockSource::new();
    let screen = build(&src, &ui(View::Lessons));
    let Some((items, selected)) = screen.body.selectable_list() else { panic!("a list of lessons") };
    assert_eq!((items.len(), selected), (3, 0));
    assert!(item_text(&items[0]).starts_with("Agent worktrees start from the default branch"));
    let moment: String = items[0].sub.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(moment, "when: before spawning an agent in its own worktree");
    // The board shows only the last week; the lessons view shows all in force.
    assert_eq!(src.board().lessons.len(), 2);
}

// --- The axolotl ------------------------------------------------------------

#[test]
fn the_axolotl_follows_what_is_owed() {
    let src = MockSource::new();
    let now = src.now();
    let mood = board_mood(&src.board(), now);
    assert_eq!(mood.state, MascotState::Worried);
    assert_eq!(mood.caption(), "worried · a question has waited 2 days");
    src.answer("ask:importer-flag", "behind a flag").unwrap();
    let mood = board_mood(&src.board(), now);
    assert_eq!(mood.caption(), "listening · 1 question waits for you");
    src.answer("ask:cover-logo", "the second").unwrap();
    assert_eq!(board_mood(&src.board(), now).caption(), "happy · nothing waits on you");
    assert_eq!(board_mood(&MockSource::empty().board(), now).state, MascotState::Idle);
}

#[test]
fn the_demo_key_cycles_every_state_and_returns() {
    let src = MockSource::new();
    let mut ui = UiState::new(true);
    for state in MascotState::ALL {
        ui.apply(Action::CycleMascot, &src);
        assert_eq!(build(&src, &ui).mascot(), Some(state));
    }
    ui.apply(Action::CycleMascot, &src);
    assert_eq!(build(&src, &ui).mascot(), Some(MascotState::Worried), "back to the real mood");
}

// --- Moving around ----------------------------------------------------------

#[test]
fn the_selection_walks_from_questions_into_work_and_stops_at_the_ends() {
    let src = MockSource::new();
    let mut ui = UiState::new(false);
    assert_eq!(ui.apply(Action::Up, &src), Effect::None);
    for expected in 1..=4 {
        assert_eq!(ui.apply(Action::Down, &src), Effect::Redraw);
        assert_eq!(ui.selected(View::Board), expected);
    }
    assert_eq!(ui.apply(Action::Down, &src), Effect::None, "five rows, no wrap");
    let board = src.board();
    assert!(matches!(picked(&board, 1), Some(Picked::Ask(_))));
    assert!(matches!(picked(&board, 2), Some(Picked::Work(_))));
    assert!(matches!(picked(&board, 99), Some(Picked::Work(w)) if w.id == "work:pricing-page"));
}

#[test]
fn back_closes_what_is_open_and_then_returns_to_the_board() {
    let src = MockSource::new();
    let mut ui = ui(View::Lessons);
    ui.apply(Action::Open, &src);
    assert!(ui.detail);
    ui.apply(Action::Back, &src);
    assert!(!ui.detail && ui.view == View::Lessons);
    ui.apply(Action::Back, &src);
    assert_eq!(ui.view, View::Board);
    assert_eq!(ui.apply(Action::Back, &src), Effect::None);
    // Nothing to open in an empty memory.
    assert_eq!(UiState::new(false).apply(Action::Open, &MockSource::empty()), Effect::None);
}

#[test]
fn a_notice_lasts_until_the_next_key() {
    let src = MockSource::new();
    let mut ui = UiState::new(false);
    ui.select(View::Board, 4);
    ui.apply(Action::Answer, &src);
    assert!(build(&src, &ui).status[0].text.starts_with("select a question"));
    assert_eq!(ui.apply(Action::Down, &src), Effect::Redraw, "the notice clears even when nothing moves");
    assert!(ui.notice.is_none());
}
