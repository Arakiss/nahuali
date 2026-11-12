use nahuali_view::mock::MockSource;
use nahuali_view::model::{Icon, MascotState, Node};
use nahuali_view::screens::{Mood, Need, arbitrate, needs_of, needs_you};
use nahuali_view::source::{
    DecisionCandidate, Extensions, PendingUpdate, Principal, PrincipalKind, RecordKind,
};
use nahuali_view::{Action, Effect, UiState, View, ViewSource, build};

fn ui(view: View) -> UiState {
    let mut state = UiState::new(false);
    state.view = view;
    state
}

#[test]
fn today_shows_a_worried_axolotl_with_its_reason() {
    let src = MockSource::new();
    let screen = build(&src, &ui(View::Today));
    assert_eq!(screen.active_tab, 0);
    assert_eq!(screen.tabs.len(), 6);
    assert_eq!(screen.mascot(), Some(MascotState::Worried));
    let Node::Row(columns) = &screen.body else {
        panic!("Today is a row");
    };
    let Node::Col(sidebar) = &columns[0] else {
        panic!("sidebar is a column");
    };
    match &sidebar[0] {
        Node::Mascot { caption, .. } => assert_eq!(caption, "worried · 2 overdue · 1 review"),
        other => panic!("expected the mascot first, got {other:?}"),
    }
}

#[test]
fn needs_you_lists_overdue_then_reviews_then_soon() {
    let src = MockSource::new();
    let rows = needs_of(&src, src.now());
    let texts: Vec<String> =
        rows.iter().map(|(item, _)| item.text.iter().map(|s| s.text.as_str()).collect()).collect();
    assert_eq!(texts[0], "Renew the TLS certificate on the staging box");
    assert_eq!(texts[1], "Send the October invoice reminders");
    assert!(texts[2].starts_with("Review: "));
    assert_eq!(texts[3], "Write the v1 import report");
    assert_eq!(rows[0].0.meta[0].text, "3d overdue");
}

#[test]
fn needs_you_explains_stale_worry_with_important_intentions_only() {
    use nahuali_view::source::{Intention, IntentionStatus, Priority};
    let src = MockSource::new();
    let mut b = src.briefing();
    let stale = |id: &str, priority| Intention {
        id: id.into(),
        text: format!("stale {id}"),
        priority,
        due: None,
        project: None,
        status: IntentionStatus::Open,
        closed_at: None,
    };
    b.stale = vec![
        stale("normal", Priority::Normal),
        stale("high", Priority::High),
        stale("critical", Priority::Critical),
        stale("high-2", Priority::High),
        stale("high-3", Priority::High),
    ];
    let texts: Vec<String> = needs_you(&b, &[], None, src.now())
        .iter()
        .filter(|(item, _)| item.meta.first().is_some_and(|m| m.text == "stale"))
        .map(|(item, _)| item.text.iter().map(|s| s.text.as_str()).collect())
        .collect();
    assert_eq!(texts.len(), 3, "at most three stale rows");
    assert_eq!(texts[0], "stale critical", "critical first");
    assert!(!texts.contains(&"stale normal".to_string()), "normal priority stays out");
}

#[test]
fn alarm_beats_every_other_mood() {
    let src = MockSource::new();
    src.set_chain_broken(true);
    for view in View::ALL {
        let screen = build(&src, &ui(view));
        if view == View::Search {
            continue;
        }
        assert_eq!(screen.mascot(), Some(MascotState::Alarmed), "{view:?}");
    }
    let mood = arbitrate(vec![]);
    assert_eq!(mood.state, MascotState::Idle);
    let mood = arbitrate(vec![
        Mood { state: MascotState::Proud, reason: "x".into() },
        Mood { state: MascotState::Worried, reason: "y".into() },
    ]);
    assert_eq!(mood.caption(), "worried · y");
}

#[test]
fn health_shows_the_alarm_band_only_when_the_chain_breaks() {
    let src = MockSource::new();
    let calm = build(&src, &ui(View::Health));
    assert!(calm.body.mascot().is_none());
    assert!(matches!(calm.status[0].icon, Some(Icon::Mascot(MascotState::Worried))));
    src.set_chain_broken(true);
    let alarmed = build(&src, &ui(View::Health));
    assert_eq!(alarmed.body.mascot(), Some(MascotState::Alarmed));
}

#[test]
fn search_ranks_and_explains() {
    let src = MockSource::new();
    let mut state = ui(View::Search);
    for c in "why did we keep the store local".chars() {
        assert_eq!(state.type_char(c), Effect::Redraw);
    }
    let page = src.recall(&state.query, 20);
    assert!(page.items.len() >= 3);
    assert_eq!(page.items[0].kind, RecordKind::Decision);
    assert!(page.items.windows(2).all(|w| w[0].score >= w[1].score));
    let screen = build(&src, &state);
    assert_eq!(screen.mascot(), Some(MascotState::Thinking));
    let (items, selected) = screen.body.selectable_list().expect("results list");
    assert_eq!(selected, 0);
    assert_eq!(items.len(), page.items.len());
}

#[test]
fn navigation_is_clamped_and_wraps_views() {
    let src = MockSource::new();
    let mut state = ui(View::Today);
    assert_eq!(state.apply(Action::Up, &src), Effect::None);
    for _ in 0..50 {
        state.apply(Action::Down, &src);
    }
    let len = needs_of(&src, src.now()).len();
    assert_eq!(state.selected(View::Today), len - 1);
    assert_eq!(state.apply(Action::PrevView, &src), Effect::Redraw);
    assert_eq!(state.view, View::Proposals);
    assert_eq!(state.apply(Action::Back, &src), Effect::Redraw);
    assert_eq!(state.view, View::Today);
    assert_eq!(state.apply(Action::Quit, &src), Effect::Quit);
}

#[test]
fn open_jumps_to_the_intention_and_close_removes_it() {
    let src = MockSource::new();
    let mut state = ui(View::Today);
    assert_eq!(state.apply(Action::Open, &src), Effect::Redraw);
    assert_eq!(state.view, View::Intentions);
    assert_eq!(state.column, 0);
    state.apply(Action::Close, &src);
    assert_eq!(state.notice.as_deref(), Some("closed · Renew the TLS certificate on the staging box"));
    let screen = build(&src, &state);
    assert_eq!(screen.status[0].text, state.notice.clone().unwrap());
    let rows = needs_of(&src, src.now());
    assert_eq!(rows.len(), 5, "six needs before, one closed");
}

#[test]
fn demo_cycle_visits_every_state_then_returns_to_the_mind() {
    let src = MockSource::new();
    let mut state = UiState::new(true);
    let mut seen = Vec::new();
    for _ in 0..MascotState::ALL.len() {
        state.apply(Action::CycleMascot, &src);
        seen.push(build(&src, &state).mascot().unwrap());
    }
    assert_eq!(seen, MascotState::ALL.to_vec());
    state.apply(Action::CycleMascot, &src);
    assert_eq!(state.mascot_override, None);
}

#[test]
fn journal_lists_days_and_shows_the_selected_one() {
    let src = MockSource::new();
    let mut state = ui(View::Journal);
    let screen = build(&src, &state);
    let Node::Row(cols) = &screen.body else { panic!() };
    match &cols[1] {
        Node::Card { title, .. } => assert_eq!(title, "Wed 4 Nov 2026"),
        other => panic!("{other:?}"),
    }
    state.apply(Action::Down, &src);
    let screen = build(&src, &state);
    let Node::Row(cols) = &screen.body else { panic!() };
    match &cols[1] {
        Node::Card { title, .. } => assert_eq!(title, "Tue 3 Nov 2026"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn proposals_refuse_approval_when_a_gate_failed_and_rejection_needs_a_reason() {
    let src = MockSource::new();
    let mut state = ui(View::Proposals);
    // The second proposal edited the eval: its frozen gate failed.
    state.apply(Action::Down, &src);
    assert_eq!(state.apply(Action::Approve, &src), Effect::Redraw);
    assert!(state.prompt.is_none());
    assert!(state.notice.as_deref().unwrap().contains("cannot approve"), "{:?}", state.notice);

    state.apply(Action::Reject, &src);
    assert!(state.prompt.is_some());
    state.prompt_key("enter", None, &src);
    assert_eq!(state.notice.as_deref(), Some("a rejection needs a reason"));
    for c in "edits the eval".chars() {
        state.prompt_key(&c.to_string(), Some(c), &src);
    }
    state.prompt_key("enter", None, &src);
    assert!(state.prompt.is_none());
    assert_eq!(src.loops().unwrap().proposals()[1].status, "rejected");
    assert!(state.notice.as_deref().unwrap().starts_with("rejected · "));
}

#[test]
fn proposals_approve_records_the_decision_and_esc_cancels() {
    let src = MockSource::new();
    let mut state = ui(View::Proposals);
    state.apply(Action::Approve, &src);
    assert_eq!(state.prompt_key("esc", None, &src), Effect::Redraw);
    assert!(state.prompt.is_none());
    assert_eq!(src.loops().unwrap().proposals()[0].status, "pending");

    state.apply(Action::Approve, &src);
    let screen = build(&src, &state);
    assert!(screen.keys.iter().any(|k| k.key == "enter" && k.label == "record"), "{:?}", screen.keys);
    assert!(screen.status[0].text.starts_with("approve nahuali:recall-ranking"));
    state.prompt_key("enter", None, &src);
    assert_eq!(src.loops().unwrap().proposals()[0].status, "approved");
    // A decided proposal cannot be decided again.
    state.apply(Action::Reject, &src);
    assert!(state.notice.as_deref().unwrap().contains("already approved"));
}

#[test]
fn proposals_show_gates_and_the_diff_on_enter() {
    let src = MockSource::new();
    let mut state = ui(View::Proposals);
    let screen = build(&src, &state);
    assert_eq!(screen.active_tab, 5);
    assert!(matches!(screen.body, Node::Row(_)));
    state.apply(Action::Open, &src);
    let screen = build(&src, &state);
    let Node::Col(parts) = &screen.body else { panic!("detail is a column") };
    assert!(parts.iter().any(|n| matches!(n, Node::Card { title, .. } if title == "Diff")));
    state.apply(Action::Back, &src);
    assert!(!state.detail);
    assert_eq!(state.view, View::Proposals);
}

fn candidate(id: &str, q: &str) -> DecisionCandidate {
    DecisionCandidate { id: id.into(), question: q.into(), chosen: "A".into() }
}

#[test]
fn a_decision_candidate_is_accepted_or_dismissed_from_today() {
    let src = MockSource::new().with_candidates(vec![candidate("c1", "A or B?"), candidate("c2", "C or D?")]);
    let rows = needs_of(&src, src.now());
    let first = rows
        .iter()
        .position(|(item, _)| item.text.iter().any(|s| s.text.contains("A or B?")))
        .expect("a candidate row");
    assert!(first > 0 && first < rows.len() - 1, "after what is overdue, before the rest");
    let mut state = ui(View::Today);
    state.apply(Action::Approve, &src);
    assert_eq!(
        state.notice.as_deref(),
        Some("no decision candidate selected"),
        "an intention is not a candidate"
    );
    state.select(View::Today, first);
    assert_eq!(state.apply(Action::Approve, &src), Effect::Redraw);
    assert_eq!(state.notice.as_deref(), Some("recorded decision:C1 · A or B?"));
    state.apply(Action::Reject, &src);
    assert_eq!(state.notice.as_deref(), Some("dismissed · C or D?"));
    assert!(src.candidates().unwrap().pending().is_empty());
}

#[test]
fn a_staged_update_tops_today_and_u_installs_it() {
    let update =
        PendingUpdate { headline: "Refile records (+1 more)".into(), commit: Some("f9c53ec".into()) };
    let src = MockSource::new().with_update(Some(update));
    let rows = needs_of(&src, src.now());
    assert!(
        rows[0].0.text.iter().any(|s| s.text == "Update ready: Refile records (+1 more)"),
        "first, before anything overdue"
    );
    let mut state = ui(View::Today);
    assert_eq!(state.apply(Action::Update, &src), Effect::InstallUpdate);
    assert_eq!(state.apply(Action::Open, &src), Effect::Redraw);
    assert!(state.notice.as_deref().is_some_and(|n| n.starts_with("u installs it")));

    let none = MockSource::new();
    assert!(!needs_of(&none, none.now()).iter().any(|(_, need)| *need == Need::Update));
    assert_eq!(state.apply(Action::Update, &none), Effect::Redraw);
    assert_eq!(state.notice.as_deref(), Some("no update is waiting"));
    let mut elsewhere = ui(View::Journal);
    assert_eq!(elsewhere.apply(Action::Update, &src), Effect::None, "only Today installs");
}

/// The core alone: no Proposals tab, no loops card, no candidate or update
/// rows, and the keys that would reach them do nothing.
#[test]
fn without_extensions_nothing_of_them_shows_or_answers() {
    let update = PendingUpdate { headline: "x".into(), commit: None };
    let src = MockSource::new()
        .with_extensions(Extensions::NONE)
        .with_candidates(vec![candidate("c1", "A or B?")])
        .with_update(Some(update));
    let today = build(&src, &ui(View::Today));
    assert_eq!(today.tabs, ["Today", "Journal", "Intentions", "Health", "Search"]);
    assert!(today.keys.iter().any(|k| k.key == "1-5"), "{:?}", today.keys);
    let needs = needs_of(&src, src.now());
    assert!(needs.iter().all(|(_, n)| matches!(n, Need::Intention { .. } | Need::Decision { .. })));

    let mut state = ui(View::Today);
    assert_eq!(state.apply(Action::Update, &src), Effect::None);
    assert_eq!(state.apply(Action::GoTo(View::Proposals), &src), Effect::None);
    assert_eq!(state.view, View::Today);
    assert_eq!(state.apply(Action::PrevView, &src), Effect::Redraw);
    assert_eq!(state.view, View::Search, "tab order skips the missing view");

    let health = build(&src, &ui(View::Health));
    fn has_card(node: &Node, title: &str) -> bool {
        match node {
            Node::Card { title: t, body, .. } => t == title || has_card(body, title),
            Node::Col(children) | Node::Row(children) => children.iter().any(|c| has_card(c, title)),
            _ => false,
        }
    }
    assert!(!has_card(&health.body, "Loops"));
    assert!(has_card(&build(&MockSource::new(), &ui(View::Health)).body, "Loops"));

    // A state left on a missing view renders Today instead of an empty pane.
    let stuck = build(&src, &ui(View::Proposals));
    assert_eq!(stuck.active_tab, 0);
    assert!(has_card(&stuck.body, "Needs attention"));
}

#[test]
fn the_screen_names_its_principal_and_measures_from_its_last_visit() {
    let own = build(&MockSource::new(), &ui(View::Today));
    assert_eq!(own.who, "ada");
    assert_eq!(own.title, "nahuali · ada · Today");
    let team = MockSource::new().with_principal(Principal::new("infra", PrincipalKind::Team));
    let screen = build(&team, &ui(View::Today));
    assert_eq!(screen.who, "infra · team");
    let Node::Row(columns) = &screen.body else { panic!("Today is a row") };
    let Node::Col(sidebar) = &columns[0] else { panic!("sidebar is a column") };
    match &sidebar[1] {
        Node::Card { title, .. } => assert_eq!(title, "Since last visit · 21h ago"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn enter_on_a_result_opens_the_record_and_its_history() {
    let src = MockSource::new();
    let mut state = ui(View::Search);
    assert_eq!(state.apply(Action::Open, &src), Effect::None, "nothing to open without a query");
    for c in "store local".chars() {
        state.type_char(c);
    }
    assert_eq!(state.apply(Action::Open, &src), Effect::Redraw);
    assert!(state.detail);
    let screen = build(&src, &state);
    let Node::Col(parts) = &screen.body else { panic!() };
    let Node::Row(panes) = &parts[1] else { panic!() };
    let Node::Card { title, .. } = &panes[1] else { panic!() };
    assert_eq!(title, "decision");
    assert_eq!(state.apply(Action::Back, &src), Effect::Redraw);
    assert!(!state.detail);
    assert_eq!(state.view, View::Search, "esc closes the record first");
    state.apply(Action::Open, &src);
    state.type_char('s');
    assert!(!state.detail, "typing goes back to the results");
}
