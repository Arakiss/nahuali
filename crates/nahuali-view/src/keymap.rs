//! One table of key bindings per view. The same table resolves key presses,
//! generates the footer hints and the `?` help card, so the UI can never
//! advertise a key that does nothing. Bindings that belong to an extension
//! exist only when the source has it ([`Extensions`]).
//!
//! Keys are canonical strings: a single character (`"j"`, `"?"`, `"1"`) or a
//! named key (`"enter"`, `"esc"`, `"tab"`, `"backtab"`, `"up"`, `"down"`,
//! `"left"`, `"right"`, `"backspace"`, `"ctrl+c"`, `"ctrl+u"`). Renderers turn
//! their native key events into these strings.

use crate::model::{KeyHint, Node, Tone};
use crate::source::Extensions;

/// Every screen, in tab order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum View {
    #[default]
    Today,
    Journal,
    Intentions,
    Health,
    Search,
    /// Extension: improvement loops.
    Proposals,
}

impl View {
    pub const ALL: [Self; 6] =
        [Self::Today, Self::Journal, Self::Intentions, Self::Health, Self::Search, Self::Proposals];

    pub fn title(self) -> &'static str {
        match self {
            Self::Today => "Today",
            Self::Journal => "Journal",
            Self::Intentions => "Intentions",
            Self::Health => "Health",
            Self::Search => "Search",
            Self::Proposals => "Proposals",
        }
    }

    /// Position in [`View::ALL`], stable whatever the extensions.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|v| *v == self).unwrap_or(0)
    }

    /// Whether a source with `ext` has this view.
    pub fn available(self, ext: Extensions) -> bool {
        self != Self::Proposals || ext.loops
    }

    /// The views a source with `ext` has, in tab order.
    pub fn shown(ext: Extensions) -> Vec<Self> {
        Self::ALL.into_iter().filter(|v| v.available(ext)).collect()
    }

    /// Position among the views shown.
    pub fn tab(self, ext: Extensions) -> usize {
        Self::shown(ext).iter().position(|v| *v == self).unwrap_or(0)
    }

    pub fn next(self, ext: Extensions) -> Self {
        let shown = Self::shown(ext);
        shown[(self.tab(ext) + 1) % shown.len()]
    }

    pub fn prev(self, ext: Extensions) -> Self {
        let shown = Self::shown(ext);
        shown[(self.tab(ext) + shown.len() - 1) % shown.len()]
    }

    /// Whether printable keys type into a text field on this view.
    pub fn takes_text(self) -> bool {
        self == Self::Search
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
    Left,
    Right,
    Open,
    Close,
    Back,
    Help,
    Quit,
    FocusSearch,
    DeleteChar,
    ClearQuery,
    /// Demo only: cycle the axolotl through every state.
    CycleMascot,
    /// Demo only: flip the mock chain between verified and broken.
    ToggleAlarm,
    /// Proposals: start an approval (asks for an optional reason). Today:
    /// accept the decision candidate.
    Approve,
    /// Proposals: start a rejection (asks for the reason). Today: dismiss the
    /// decision candidate.
    Reject,
    /// Proposals: start deferring.
    Defer,
    /// Today: install the staged update.
    Update,
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

/// Navigation every view understands, including the text-taking ones.
const NAV: &[Binding] = &[
    bind(&["tab"], Action::NextView, "next view", None),
    bind(&["backtab"], Action::PrevView, "previous view", None),
    bind(&["ctrl+c"], Action::Quit, "quit", None),
];

/// The digit keys, one per view shown, and the hint that covers them.
const DIGITS: [&[&str]; 6] = [&["1"], &["2"], &["3"], &["4"], &["5"], &["6"]];
const DIGIT_HINTS: [&str; 6] = ["1", "1-2", "1-3", "1-4", "1-5", "1-6"];

/// Single-key globals besides the digits, only on views that do not take text.
const GLOBAL: &[Binding] = &[
    bind(&["/"], Action::FocusSearch, "search", Some(("/", "search"))),
    bind(&["?"], Action::Help, "keys", Some(("?", "help"))),
    bind(&["q"], Action::Quit, "quit", Some(("q", "quit"))),
    demo(&["m"], Action::CycleMascot, "mascot"),
    demo(&["!"], Action::ToggleAlarm, "alarm"),
];

const TODAY: &[Binding] = &[
    bind(&["j", "down"], Action::Down, "next item", MOVE),
    bind(&["k", "up"], Action::Up, "previous item", MOVE),
    bind(&["enter"], Action::Open, "open in its view", Some(("enter", "open"))),
    bind(&["c"], Action::Close, "close intention", Some(("c", "close"))),
];

// No footer hint: the candidate's own row says "a accept · x dismiss", and
// the footer keeps its room for the health line.
const TODAY_CANDIDATES: &[Binding] = &[
    bind(&["a"], Action::Approve, "accept the decision candidate", None),
    bind(&["x"], Action::Reject, "dismiss the decision candidate", None),
];

const TODAY_UPDATES: &[Binding] = &[bind(&["u"], Action::Update, "install the staged update", None)];

const JOURNAL: &[Binding] = &[
    bind(&["j", "down"], Action::Down, "older day", Some(("j k", "day"))),
    bind(&["k", "up"], Action::Up, "newer day", Some(("j k", "day"))),
];

const INTENTIONS: &[Binding] = &[
    bind(&["h", "left"], Action::Left, "previous column", Some(("h l", "column"))),
    bind(&["l", "right"], Action::Right, "next column", Some(("h l", "column"))),
    bind(&["j", "down"], Action::Down, "next item", MOVE),
    bind(&["k", "up"], Action::Up, "previous item", MOVE),
    bind(&["c"], Action::Close, "close intention", Some(("c", "close"))),
];

const HEALTH: &[Binding] = &[];

const SEARCH: &[Binding] = &[
    bind(&["down"], Action::Down, "next result", Some(("↑ ↓", "move"))),
    bind(&["up"], Action::Up, "previous result", Some(("↑ ↓", "move"))),
    bind(&["enter"], Action::Open, "show the record and its history", Some(("enter", "open"))),
    bind(&["backspace"], Action::DeleteChar, "delete a character", None),
    bind(&["ctrl+u"], Action::ClearQuery, "clear the query", Some(("^u", "clear"))),
    bind(&["esc"], Action::Back, "close the record, clear, then back to Today", Some(("esc", "back"))),
];

const PROPOSALS: &[Binding] = &[
    bind(&["j", "down"], Action::Down, "next proposal", MOVE),
    bind(&["k", "up"], Action::Up, "previous proposal", MOVE),
    bind(&["enter"], Action::Open, "show or hide the diff", Some(("enter", "diff"))),
    bind(&["a"], Action::Approve, "approve", Some(("a", "approve"))),
    bind(&["x"], Action::Reject, "reject with a reason", Some(("x", "reject"))),
    bind(&["d"], Action::Defer, "defer", Some(("d", "defer"))),
    bind(&["esc"], Action::Back, "back to the list, then Today", None),
];

/// Bindings while a decision prompt is open: the text goes into the reason.
pub const PROMPT: &[Binding] = &[
    bind(&["enter"], Action::Open, "record the decision", Some(("enter", "record"))),
    bind(&["backspace"], Action::DeleteChar, "delete a character", None),
    bind(&["esc"], Action::Back, "cancel", Some(("esc", "cancel"))),
    bind(&["ctrl+c"], Action::Quit, "quit", None),
];

/// Every binding active on `view` for a source with `ext`, view-specific first.
pub fn bindings(view: View, demo: bool, ext: Extensions) -> Vec<Binding> {
    let mut own: Vec<Binding> = match view {
        View::Today => TODAY.to_vec(),
        View::Journal => JOURNAL.to_vec(),
        View::Intentions => INTENTIONS.to_vec(),
        View::Health => HEALTH.to_vec(),
        View::Search => SEARCH.to_vec(),
        View::Proposals if ext.loops => PROPOSALS.to_vec(),
        View::Proposals => Vec::new(),
    };
    if view == View::Today {
        if ext.candidates {
            own.extend_from_slice(TODAY_CANDIDATES);
        }
        if ext.updates {
            own.extend_from_slice(TODAY_UPDATES);
        }
    }
    if !view.takes_text() {
        let shown = View::shown(ext);
        let hint = DIGIT_HINTS[shown.len() - 1];
        for (i, v) in shown.into_iter().enumerate() {
            own.push(bind(DIGITS[i], Action::GoTo(v), v.title(), Some((hint, "views"))));
        }
        own.extend_from_slice(GLOBAL);
    }
    own.extend_from_slice(NAV);
    own.into_iter().filter(|b| demo || !b.demo).collect()
}

/// The action bound to `key` on `view`, if any.
pub fn resolve(view: View, key: &str, demo: bool, ext: Extensions) -> Option<Action> {
    bindings(view, demo, ext).into_iter().find(|b| b.keys.contains(&key)).map(|b| b.action)
}

/// Footer hints for `view`, in binding order, merged and deduplicated.
pub fn footer_hints(view: View, demo: bool, ext: Extensions) -> Vec<KeyHint> {
    let mut hints: Vec<KeyHint> = Vec::new();
    for binding in bindings(view, demo, ext) {
        let Some((key, label)) = binding.hint else {
            continue;
        };
        if !hints.iter().any(|h| h.key == key && h.label == label) {
            hints.push(KeyHint { key: key.to_string(), label: label.to_string() });
        }
    }
    hints
}

/// Human name for a canonical key.
pub fn display_key(key: &str) -> &str {
    match key {
        "up" => "↑",
        "down" => "↓",
        "left" => "←",
        "right" => "→",
        "backtab" => "shift+tab",
        other => other,
    }
}

/// The `?` help card: every binding of the view, with all its keys.
pub fn help(view: View, demo: bool, ext: Extensions) -> Node {
    let mut rows: Vec<(String, String)> = Vec::new();
    for binding in bindings(view, demo, ext) {
        let keys = binding.keys.iter().map(|k| display_key(k)).collect::<Vec<_>>().join(" / ");
        rows.push((keys, binding.label.to_string()));
    }
    if view.takes_text() {
        rows.insert(0, ("type".to_string(), "edit the query".to_string()));
    }
    Node::card(format!("Keys · {}", view.title()), Tone::Accent, Node::Kv(rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY: [Extensions; 2] = [Extensions::ALL, Extensions::NONE];

    #[test]
    fn no_view_binds_a_key_twice() {
        for ext in EVERY {
            for view in View::ALL {
                let mut seen = std::collections::HashSet::new();
                for binding in bindings(view, true, ext) {
                    for key in binding.keys {
                        assert!(seen.insert(*key), "{view:?} binds {key} twice");
                    }
                }
            }
        }
    }

    #[test]
    fn every_hint_names_keys_that_resolve() {
        for ext in EVERY {
            for view in View::shown(ext) {
                for hint in footer_hints(view, true, ext) {
                    for token in hint.key.split(' ') {
                        let key = match token {
                            "↑" => "up",
                            "↓" => "down",
                            "^u" => "ctrl+u",
                            other => other.split('-').next_back().unwrap_or(other),
                        };
                        assert!(
                            resolve(view, key, true, ext).is_some(),
                            "{view:?} shows '{}' but {key} is unbound",
                            hint.key
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_view_can_leave_and_quit() {
        for ext in EVERY {
            for view in View::shown(ext) {
                assert_eq!(resolve(view, "tab", false, ext), Some(Action::NextView));
                assert_eq!(resolve(view, "ctrl+c", false, ext), Some(Action::Quit));
            }
        }
    }

    #[test]
    fn text_views_do_not_steal_printable_keys() {
        for key in ["q", "j", "k", "/", "?", "1", "m"] {
            assert_eq!(resolve(View::Search, key, true, Extensions::ALL), None, "{key}");
        }
    }

    #[test]
    fn demo_bindings_stay_hidden_unless_asked() {
        let ext = Extensions::ALL;
        assert_eq!(resolve(View::Today, "m", false, ext), None);
        assert_eq!(resolve(View::Today, "m", true, ext), Some(Action::CycleMascot));
        assert!(!footer_hints(View::Today, false, ext).iter().any(|h| h.key == "m"));
    }

    #[test]
    fn hints_merge_shared_labels() {
        let hints = footer_hints(View::Today, false, Extensions::ALL);
        assert_eq!(hints.iter().filter(|h| h.label == "move").count(), 1);
        assert_eq!(hints.iter().filter(|h| h.label == "views").count(), 1);
        assert!(hints.iter().any(|h| h.key == "1-6"));
    }

    #[test]
    fn an_absent_extension_leaves_no_view_and_no_key() {
        let core = Extensions::NONE;
        assert_eq!(View::shown(core).len(), 5);
        assert!(!View::Proposals.available(core));
        assert_eq!(View::Search.next(core), View::Today, "tab skips the missing view");
        assert_eq!(View::Today.prev(core), View::Search);
        for key in ["a", "x", "u"] {
            assert_eq!(resolve(View::Today, key, true, core), None, "{key}");
        }
        assert_eq!(resolve(View::Today, "6", true, core), None);
        assert_eq!(resolve(View::Today, "5", true, core), Some(Action::GoTo(View::Search)));
        assert!(footer_hints(View::Today, false, core).iter().any(|h| h.key == "1-5"));
        let only_updates = Extensions { updates: true, ..core };
        assert_eq!(resolve(View::Today, "u", false, only_updates), Some(Action::Update));
        assert_eq!(resolve(View::Today, "a", false, only_updates), None);
    }
}
