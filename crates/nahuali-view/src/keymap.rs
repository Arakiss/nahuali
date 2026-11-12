//! One table of key bindings per view. The same table resolves key presses,
//! generates the footer hints and the `?` help card, so the UI can never
//! advertise a key that does nothing.
//!
//! Keys are canonical strings: a single character (`"j"`, `"?"`, `"1"`) or a
//! named key (`"enter"`, `"esc"`, `"tab"`, `"backtab"`, `"up"`, `"down"`,
//! `"backspace"`, `"ctrl+c"`). Renderers turn their native key events into
//! these strings.

use crate::model::{KeyHint, Node, Tone};

/// Every screen, in tab order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum View {
    /// Where things stand and what is owed.
    #[default]
    Board,
    /// What was learned, and when each lesson applies.
    Lessons,
    /// Ask the memory what it knows about something.
    Recall,
}

impl View {
    pub const ALL: [Self; 3] = [Self::Board, Self::Lessons, Self::Recall];

    pub fn title(self) -> &'static str {
        match self {
            Self::Board => "Board",
            Self::Lessons => "Lessons",
            Self::Recall => "Recall",
        }
    }

    /// Position in [`View::ALL`], which is also the tab.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(0)
    }

    pub fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// Whether printable keys type into a text field on this view.
    pub fn takes_text(self) -> bool {
        matches!(self, Self::Recall)
    }
}

/// Everything a key can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    NextView,
    PrevView,
    GoTo(View),
    Down,
    Up,
    /// Show the selected item with its past and its marks.
    Open,
    Back,
    /// Board: start answering the selected question.
    Answer,
    Help,
    Quit,
    DeleteChar,
    /// Demo only: cycle the axolotl through every state.
    CycleMascot,
}

/// One binding. `hint` is what the footer shows; bindings that share the
/// same hint (`j`/`k` both say "j k move") are merged into one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    pub keys: &'static [&'static str],
    pub action: Action,
    pub label: &'static str,
    pub hint: Option<(&'static str, &'static str)>,
    pub demo: bool,
}

const fn bind(
    keys: &'static [&'static str],
    action: Action,
    label: &'static str,
    hint: Option<(&'static str, &'static str)>,
) -> Binding {
    Binding { keys, action, label, hint, demo: false }
}

const fn demo(keys: &'static [&'static str], action: Action, label: &'static str) -> Binding {
    Binding { keys, action, label, hint: Some((keys[0], label)), demo: true }
}

const MOVE: Option<(&str, &str)> = Some(("j k", "move"));

/// Navigation every view understands.
const NAV: &[Binding] = &[
    bind(&["tab"], Action::NextView, "next view", None),
    bind(&["backtab"], Action::PrevView, "previous view", None),
    bind(&["ctrl+c"], Action::Quit, "quit", None),
];

const DIGITS: [&[&str]; 3] = [&["1"], &["2"], &["3"]];

const GLOBAL: &[Binding] = &[
    bind(&["?"], Action::Help, "keys", Some(("?", "help"))),
    bind(&["q"], Action::Quit, "quit", Some(("q", "quit"))),
    demo(&["m"], Action::CycleMascot, "mascot"),
];

const BOARD: &[Binding] = &[
    bind(&["j", "down"], Action::Down, "next item", MOVE),
    bind(&["k", "up"], Action::Up, "previous item", MOVE),
    bind(&["enter"], Action::Open, "open it with its past and its marks", Some(("enter", "open"))),
    bind(&["a"], Action::Answer, "answer the selected question", Some(("a", "answer"))),
    bind(&["esc"], Action::Back, "close what is open", Some(("esc", "back"))),
];

const LESSONS: &[Binding] = &[
    bind(&["j", "down"], Action::Down, "next lesson", MOVE),
    bind(&["k", "up"], Action::Up, "previous lesson", MOVE),
    bind(&["enter"], Action::Open, "open it with its past and its marks", Some(("enter", "open"))),
    bind(&["esc"], Action::Back, "close what is open, then back to the board", Some(("esc", "back"))),
];

/// Recall is a text field: letters, digits and `?` type into it, so only
/// keys that are not characters do anything else.
const RECALL: &[Binding] = &[
    bind(&["down"], Action::Down, "next result", Some(("up down", "move"))),
    bind(&["up"], Action::Up, "previous result", Some(("up down", "move"))),
    bind(&["enter"], Action::Open, "open it with its past and its marks", Some(("enter", "open"))),
    bind(&["backspace"], Action::DeleteChar, "delete a character", None),
    bind(
        &["esc"],
        Action::Back,
        "close what is open, then clear, then back to the board",
        Some(("esc", "back")),
    ),
    bind(&["tab"], Action::NextView, "next view", Some(("tab", "views"))),
];

/// Bindings while an answer is being typed: the text goes into the answer.
pub const PROMPT: &[Binding] = &[
    bind(&["enter"], Action::Open, "record the answer", Some(("enter", "record"))),
    bind(&["backspace"], Action::DeleteChar, "delete a character", None),
    bind(&["esc"], Action::Back, "cancel", Some(("esc", "cancel"))),
    bind(&["ctrl+c"], Action::Quit, "quit", None),
];

/// Every binding active on `view`, view-specific first.
pub fn bindings(view: View, demo: bool) -> Vec<Binding> {
    let mut own: Vec<Binding> = match view {
        View::Board => BOARD.to_vec(),
        View::Lessons => LESSONS.to_vec(),
        View::Recall => {
            let mut own = RECALL.to_vec();
            own.extend(NAV.iter().filter(|b| b.keys != ["tab"]).copied());
            return own;
        }
    };
    for (i, v) in View::ALL.into_iter().enumerate() {
        own.push(bind(DIGITS[i], Action::GoTo(v), v.title(), Some(("1-3", "views"))));
    }
    own.extend_from_slice(GLOBAL);
    own.extend_from_slice(NAV);
    own.into_iter().filter(|b| demo || !b.demo).collect()
}

/// The action bound to `key` on `view`, if any.
pub fn resolve(view: View, key: &str, demo: bool) -> Option<Action> {
    bindings(view, demo).into_iter().find(|b| b.keys.contains(&key)).map(|b| b.action)
}

fn merged(bindings: &[Binding]) -> Vec<KeyHint> {
    let mut hints: Vec<KeyHint> = Vec::new();
    for binding in bindings {
        let Some((key, label)) = binding.hint else {
            continue;
        };
        if !hints.iter().any(|h| h.key == key && h.label == label) {
            hints.push(KeyHint { key: key.to_string(), label: label.to_string() });
        }
    }
    hints
}

/// Footer hints for `view`, in binding order, merged and deduplicated.
pub fn footer_hints(view: View, demo: bool) -> Vec<KeyHint> {
    merged(&bindings(view, demo))
}

/// Footer hints while an answer is being typed.
pub fn prompt_hints() -> Vec<KeyHint> {
    merged(PROMPT)
}

/// Human name for a canonical key.
pub fn display_key(key: &str) -> &str {
    match key {
        "up" => "↑",
        "down" => "↓",
        "backtab" => "shift+tab",
        other => other,
    }
}

/// The `?` help card: every binding of the view, with all its keys.
pub fn help(view: View, demo: bool) -> Node {
    let mut rows: Vec<(String, String)> = Vec::new();
    for binding in bindings(view, demo) {
        let keys = binding.keys.iter().map(|k| display_key(k)).collect::<Vec<_>>().join(" / ");
        rows.push((keys, binding.label.to_string()));
    }
    Node::card(format!("Keys · {}", view.title()), Tone::Accent, Node::Kv(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_view_binds_a_key_twice() {
        for view in View::ALL {
            let mut seen = std::collections::HashSet::new();
            for binding in bindings(view, true) {
                for key in binding.keys {
                    assert!(seen.insert(*key), "{view:?} binds {key} twice");
                }
            }
        }
    }

    #[test]
    fn every_hint_names_keys_that_resolve() {
        for view in View::ALL {
            for hint in footer_hints(view, true) {
                for token in hint.key.split(' ') {
                    let key = token.split('-').next_back().unwrap_or(token);
                    assert!(
                        resolve(view, key, true).is_some(),
                        "{view:?} shows '{}' but {key} is unbound",
                        hint.key
                    );
                }
            }
        }
    }

    #[test]
    fn every_view_can_leave_and_quit() {
        for view in View::ALL {
            assert_eq!(resolve(view, "tab", false), Some(Action::NextView));
            assert_eq!(resolve(view, "ctrl+c", false), Some(Action::Quit));
            // Where letters are typed, `q` is a letter: ctrl+c quits.
            let q = if view.takes_text() { None } else { Some(Action::Quit) };
            assert_eq!(resolve(view, "q", false), q);
        }
        assert_eq!(View::Recall.next(), View::Board);
        assert_eq!(View::Board.prev(), View::Recall);
    }

    #[test]
    fn demo_bindings_stay_hidden_unless_asked() {
        assert_eq!(resolve(View::Board, "m", false), None);
        assert_eq!(resolve(View::Board, "m", true), Some(Action::CycleMascot));
        assert!(!footer_hints(View::Board, false).iter().any(|h| h.key == "m"));
    }

    #[test]
    fn hints_merge_shared_labels() {
        let hints = footer_hints(View::Board, false);
        assert_eq!(hints.iter().filter(|h| h.label == "move").count(), 1);
        assert_eq!(hints.iter().filter(|h| h.label == "views").count(), 1);
    }

    #[test]
    fn only_the_board_answers() {
        assert_eq!(resolve(View::Board, "a", false), Some(Action::Answer));
        assert_eq!(resolve(View::Lessons, "a", false), None);
    }
}
