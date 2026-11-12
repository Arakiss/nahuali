//! UI state and the pure transitions keys trigger. The renderer turns a key
//! into an [`Action`] through the keymap and calls [`UiState::apply`]; no
//! terminal code is involved, so every transition is unit-testable.

use crate::keymap::{Action, View};
use crate::model::MascotState;
use crate::screens::{Need, intention_columns, needs_of};
use crate::source::{Extensions, Verdict, ViewSource};

/// What the caller should do after an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Nothing visible changed.
    None,
    /// Rebuild and redraw.
    Redraw,
    /// Toggle the help overlay.
    Help,
    /// Leave the UI.
    Quit,
    /// Demo: the caller owns the mock and flips its alarm.
    ToggleAlarm,
    /// Hand the terminal back and install the pending update.
    InstallUpdate,
}

/// A verdict whose reason is being typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub verdict: Verdict,
    pub id: String,
    pub target: String,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiState {
    pub view: View,
    selected: [usize; 6],
    /// Proposals: the literal diff is shown. Search: the selected record is
    /// open with its history.
    pub detail: bool,
    /// Proposals: a decision prompt is open; keys type its reason.
    pub prompt: Option<Prompt>,
    /// Focused column on Intentions (0 overdue, 1 open, 2 closed).
    pub column: usize,
    pub query: String,
    /// A recall is running (the axolotl thinks).
    pub searching: bool,
    pub mascot_override: Option<MascotState>,
    pub demo: bool,
    /// One-line message shown first in the status line until the next key.
    pub notice: Option<String>,
}

impl UiState {
    pub fn new(demo: bool) -> Self {
        Self { demo, ..Self::default() }
    }

    /// The same state shown on `view`: what a screen falls back to when its
    /// own view is missing.
    pub fn on(&self, view: View) -> Self {
        Self { view, detail: false, prompt: None, ..self.clone() }
    }

    pub fn selected(&self, view: View) -> usize {
        self.selected[view.index()]
    }

    pub fn select(&mut self, view: View, index: usize) {
        self.selected[view.index()] = index;
    }

    fn list_len(&self, src: &dyn ViewSource) -> usize {
        let now = src.now();
        match self.view {
            View::Today => needs_of(src, now).len(),
            View::Journal => src.journal_days(now.date).len(),
            View::Intentions => intention_columns(&src.intentions(), now)[self.column.min(2)].len(),
            View::Health => 0,
            View::Proposals => src.loops().map_or(0, |l| l.proposals().len()),
            View::Search => {
                if self.query.trim().is_empty() {
                    0
                } else {
                    src.recall(&self.query, 20).items.len()
                }
            }
        }
    }

    fn step(&mut self, src: &dyn ViewSource, delta: isize) -> Effect {
        let len = self.list_len(src);
        if len == 0 {
            return Effect::None;
        }
        let current = self.selected(self.view).min(len - 1);
        let next = current.saturating_add_signed(delta).min(len - 1);
        if next == current && self.selected(self.view) == current {
            return Effect::None;
        }
        self.select(self.view, next);
        Effect::Redraw
    }

    fn go(&mut self, view: View, ext: Extensions) -> Effect {
        if self.view == view || !view.available(ext) {
            return Effect::None;
        }
        self.view = view;
        self.detail = false;
        Effect::Redraw
    }

    /// Type one character into the query (text-taking views only).
    pub fn type_char(&mut self, c: char) -> Effect {
        if !self.view.takes_text() || c.is_control() {
            return Effect::None;
        }
        self.notice = None;
        self.query.push(c);
        self.detail = false;
        self.select(View::Search, 0);
        self.searching = true;
        Effect::Redraw
    }

    /// Apply one action.
    pub fn apply(&mut self, action: Action, src: &dyn ViewSource) -> Effect {
        let had_notice = self.notice.take().is_some();
        let effect = self.apply_inner(action, src);
        if had_notice && effect == Effect::None { Effect::Redraw } else { effect }
    }

    fn apply_inner(&mut self, action: Action, src: &dyn ViewSource) -> Effect {
        let ext = Extensions::of(src);
        match action {
            Action::NextView => self.go(self.view.next(ext), ext),
            Action::PrevView => self.go(self.view.prev(ext), ext),
            Action::GoTo(view) => self.go(view, ext),
            Action::FocusSearch => self.go(View::Search, ext),
            Action::Down => self.step(src, 1),
            Action::Up => self.step(src, -1),
            Action::Left | Action::Right if self.view == View::Intentions => {
                let next = if action == Action::Left {
                    self.column.saturating_sub(1)
                } else {
                    (self.column + 1).min(2)
                };
                if next == self.column {
                    return Effect::None;
                }
                self.column = next;
                self.select(View::Intentions, 0);
                Effect::Redraw
            }
            Action::Left | Action::Right => Effect::None,
            Action::Open if self.view == View::Proposals && ext.loops => {
                self.detail = !self.detail;
                Effect::Redraw
            }
            Action::Open if self.view == View::Search => self.open_record(src),
            Action::Open => self.open(src),
            Action::Close => self.close(src),
            Action::Approve if self.view == View::Today => self.candidate(src, true),
            Action::Reject if self.view == View::Today => self.candidate(src, false),
            Action::Approve => self.start_prompt(Verdict::Approve, src),
            Action::Reject => self.start_prompt(Verdict::Reject, src),
            Action::Defer => self.start_prompt(Verdict::Defer, src),
            Action::Back if matches!(self.view, View::Proposals | View::Search) && self.detail => {
                self.detail = false;
                Effect::Redraw
            }
            Action::Back => {
                if self.query.is_empty() {
                    self.go(View::Today, ext)
                } else {
                    self.query.clear();
                    self.searching = false;
                    Effect::Redraw
                }
            }
            Action::DeleteChar => {
                if self.query.pop().is_some() {
                    self.detail = false;
                    self.select(View::Search, 0);
                    self.searching = !self.query.is_empty();
                    Effect::Redraw
                } else {
                    Effect::None
                }
            }
            Action::ClearQuery => {
                if self.query.is_empty() {
                    return Effect::None;
                }
                self.query.clear();
                self.detail = false;
                self.searching = false;
                Effect::Redraw
            }
            Action::Help => Effect::Help,
            Action::Quit => Effect::Quit,
            Action::CycleMascot => {
                self.mascot_override = Some(match self.mascot_override {
                    None => MascotState::ALL[0],
                    Some(state) if state == *MascotState::ALL.last().unwrap_or(&state) => {
                        self.mascot_override = None;
                        return Effect::Redraw;
                    }
                    Some(state) => state.next(),
                });
                Effect::Redraw
            }
            Action::ToggleAlarm => Effect::ToggleAlarm,
            Action::Update if self.view == View::Today => match src.updates() {
                Some(u) if u.staged().is_some() => Effect::InstallUpdate,
                Some(_) => {
                    self.notice = Some("no update is waiting".into());
                    Effect::Redraw
                }
                None => Effect::None,
            },
            Action::Update => Effect::None,
        }
    }

    fn start_prompt(&mut self, verdict: Verdict, src: &dyn ViewSource) -> Effect {
        let Some(loops) = src.loops().filter(|_| self.view == View::Proposals) else {
            return Effect::None;
        };
        let list = loops.proposals();
        let Some(p) = list.get(self.selected(View::Proposals).min(list.len().saturating_sub(1))) else {
            self.notice = Some("no proposal selected".into());
            return Effect::Redraw;
        };
        if !p.is_open() {
            self.notice = Some(format!("{} is already {}", p.id, p.status));
            return Effect::Redraw;
        }
        if verdict == Verdict::Approve && !p.gates_ok() {
            self.notice = Some(format!("cannot approve {}: a gate did not pass (frozen {})", p.id, p.frozen));
            return Effect::Redraw;
        }
        self.prompt =
            Some(Prompt { verdict, id: p.id.clone(), target: p.target.clone(), reason: String::new() });
        Effect::Redraw
    }

    /// Handle one key while a decision prompt is open: enter records it, esc
    /// cancels, printable characters type the reason.
    pub fn prompt_key(&mut self, key: &str, typed: Option<char>, src: &dyn ViewSource) -> Effect {
        let Some(prompt) = self.prompt.as_mut() else {
            return Effect::None;
        };
        let action = crate::keymap::PROMPT.iter().find(|b| b.keys.contains(&key)).map(|b| b.action);
        match action {
            Some(Action::Quit) => Effect::Quit,
            Some(Action::Back) => {
                self.prompt = None;
                self.notice = Some("cancelled".into());
                Effect::Redraw
            }
            Some(Action::DeleteChar) => {
                prompt.reason.pop();
                Effect::Redraw
            }
            Some(_) => {
                let reason = prompt.reason.trim().to_string();
                if prompt.verdict == Verdict::Reject && reason.is_empty() {
                    self.notice = Some("a rejection needs a reason".into());
                    return Effect::Redraw;
                }
                let (id, target, verdict) = (prompt.id.clone(), prompt.target.clone(), prompt.verdict);
                self.prompt = None;
                self.notice = Some(match src.loops().map(|l| l.decide(&id, verdict, &reason)) {
                    Some(Ok(())) => format!("{} · {target}", verdict.decision()),
                    Some(Err(error)) => format!("could not record it: {error}"),
                    None => "this mind runs no improvement loops".into(),
                });
                Effect::Redraw
            }
            None => match typed {
                Some(c) if !c.is_control() => {
                    prompt.reason.push(c);
                    Effect::Redraw
                }
                _ => Effect::None,
            },
        }
    }

    fn open(&mut self, src: &dyn ViewSource) -> Effect {
        if self.view != View::Today {
            return Effect::None;
        }
        let now = src.now();
        let needs = needs_of(src, now);
        let Some((_, need)) = needs.get(self.selected(View::Today).min(needs.len().saturating_sub(1))) else {
            return Effect::None;
        };
        match need {
            Need::Intention { id, .. } => {
                let columns = intention_columns(&src.intentions(), now);
                for (c, list) in columns.iter().enumerate() {
                    if let Some(i) = list.iter().position(|x| &x.id == id) {
                        self.column = c;
                        self.select(View::Intentions, i);
                        return self.go(View::Intentions, Extensions::of(src));
                    }
                }
                Effect::None
            }
            Need::Decision { decided_on, .. } => {
                let days = src.journal_days(*decided_on);
                if let Some(i) = days.iter().position(|d| d.date == *decided_on) {
                    self.select(View::Journal, i);
                }
                self.go(View::Journal, Extensions::of(src))
            }
            Need::Candidate { .. } => {
                self.notice = Some("a accepts it as a decision · x dismisses it".into());
                Effect::Redraw
            }
            Need::Update => {
                self.notice =
                    Some("u installs it: the TUI steps aside while it installs, then reopens".into());
                Effect::Redraw
            }
        }
    }

    /// Search: open the selected result with its history.
    fn open_record(&mut self, src: &dyn ViewSource) -> Effect {
        if self.query.trim().is_empty() {
            return Effect::None;
        }
        let page = src.recall(&self.query, 20);
        let Some(item) = page.items.get(self.selected(View::Search).min(page.items.len().saturating_sub(1)))
        else {
            return Effect::None;
        };
        if src.record(&item.id).is_none() {
            self.notice = Some(format!("{} cannot be opened here", item.id));
            return Effect::Redraw;
        }
        self.detail = true;
        self.searching = false;
        Effect::Redraw
    }

    /// Today: accept or dismiss the selected decision candidate.
    fn candidate(&mut self, src: &dyn ViewSource, accept: bool) -> Effect {
        let Some(candidates) = src.candidates() else {
            return Effect::None;
        };
        let needs = needs_of(src, src.now());
        let Some((_, Need::Candidate { id, question })) = needs.get(self.selected(View::Today)) else {
            self.notice = Some("no decision candidate selected".into());
            return Effect::Redraw;
        };
        let (id, question) = (id.clone(), question.clone());
        self.notice = Some(if accept {
            match candidates.accept(&id) {
                Ok(decision) => format!("recorded {decision} · {question}"),
                Err(e) => format!("could not accept: {e}"),
            }
        } else {
            match candidates.dismiss(&id) {
                Ok(()) => format!("dismissed · {question}"),
                Err(e) => format!("could not dismiss: {e}"),
            }
        });
        let len = self.list_len(src);
        let current = self.selected(View::Today);
        self.select(View::Today, current.min(len.saturating_sub(1)));
        Effect::Redraw
    }

    fn close(&mut self, src: &dyn ViewSource) -> Effect {
        let now = src.now();
        let target = match self.view {
            View::Today => {
                let needs = needs_of(src, now);
                match needs.get(self.selected(View::Today)) {
                    Some((_, Need::Intention { id, text })) => Some((id.clone(), text.clone())),
                    _ => None,
                }
            }
            View::Intentions if self.column < 2 => {
                let columns = intention_columns(&src.intentions(), now);
                columns[self.column]
                    .get(self.selected(View::Intentions))
                    .map(|i| (i.id.clone(), i.text.clone()))
            }
            _ => None,
        };
        let Some((id, text)) = target else {
            self.notice = Some("nothing to close here".into());
            return Effect::Redraw;
        };
        self.notice = Some(match src.close_intention(&id) {
            Ok(()) => format!("closed · {text}"),
            Err(error) => format!("could not close: {error}"),
        });
        let len = self.list_len(src);
        let current = self.selected(self.view);
        self.select(self.view, current.min(len.saturating_sub(1)));
        Effect::Redraw
    }
}
