//! UI state and the pure transitions keys trigger. Nothing here draws: an
//! [`Effect`] tells the renderer what to do next.

use crate::keymap::{Action, View};
use crate::model::MascotState;
use crate::source::{Ask, Board, ViewSource, Work};

/// What the renderer has to do after a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    Redraw,
    Help,
    Quit,
}

/// An answer being typed for one ask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub ask: String,
    pub question: String,
    pub text: String,
}

/// The selected row of the board: asks come first, then work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Picked<'a> {
    Ask(&'a Ask),
    Work(&'a Work),
}

/// The board's row at `index`, clamped to its last row.
pub fn picked(board: &Board, index: usize) -> Option<Picked<'_>> {
    let len = board.asks.len() + board.works.len();
    if len == 0 {
        return None;
    }
    let i = index.min(len - 1);
    match board.asks.get(i) {
        Some(ask) => Some(Picked::Ask(ask)),
        None => board.works.get(i - board.asks.len()).map(Picked::Work),
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiState {
    pub view: View,
    selected: [usize; 3],
    /// The selected item is open with its past and its marks.
    pub detail: bool,
    /// An answer is being typed; keys go into it.
    pub prompt: Option<Prompt>,
    /// What is being asked on the recall view.
    pub query: String,
    /// The query just changed: renderers show the axolotl thinking.
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

    /// The same state shown on `view`.
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
        match self.view {
            View::Board => {
                let board = src.board();
                board.asks.len() + board.works.len()
            }
            View::Lessons => src.lessons().len(),
            View::Recall if self.query.trim().is_empty() => 0,
            View::Recall => src.recall(&self.query).len(),
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

    fn go(&mut self, view: View) -> Effect {
        if self.view == view {
            return Effect::None;
        }
        self.view = view;
        self.detail = false;
        Effect::Redraw
    }

    /// Type one character into the recall view's question.
    pub fn type_char(&mut self, c: char) -> Effect {
        if self.view != View::Recall || self.detail || c.is_control() {
            return Effect::None;
        }
        self.notice = None;
        self.query.push(c);
        self.select(View::Recall, 0);
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
        match action {
            Action::NextView => self.go(self.view.next()),
            Action::PrevView => self.go(self.view.prev()),
            Action::GoTo(view) => self.go(view),
            Action::Down => self.step(src, 1),
            Action::Up => self.step(src, -1),
            Action::Open => {
                if self.list_len(src) == 0 {
                    return Effect::None;
                }
                self.detail = !self.detail;
                Effect::Redraw
            }
            Action::Back if self.detail => {
                self.detail = false;
                Effect::Redraw
            }
            Action::Back if self.view == View::Recall && !self.query.is_empty() => {
                self.query.clear();
                self.select(View::Recall, 0);
                Effect::Redraw
            }
            Action::Back => self.go(View::Board),
            Action::Answer => self.start_answer(src),
            Action::DeleteChar if self.view == View::Recall && !self.detail => {
                if self.query.pop().is_none() {
                    return Effect::None;
                }
                self.select(View::Recall, 0);
                self.searching = true;
                Effect::Redraw
            }
            Action::DeleteChar => Effect::None,
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
        }
    }

    fn start_answer(&mut self, src: &dyn ViewSource) -> Effect {
        if self.view != View::Board {
            return Effect::None;
        }
        let board = src.board();
        match picked(&board, self.selected(View::Board)) {
            Some(Picked::Ask(ask)) => {
                self.prompt =
                    Some(Prompt { ask: ask.id.clone(), question: ask.question.clone(), text: String::new() });
            }
            Some(Picked::Work(_)) => {
                self.notice = Some("select a question to answer it".into());
            }
            None => self.notice = Some("nothing waits on you".into()),
        }
        Effect::Redraw
    }

    /// Handle one key while an answer is being typed: enter records it, esc
    /// cancels, printable characters type it.
    pub fn prompt_key(&mut self, key: &str, typed: Option<char>, src: &dyn ViewSource) -> Effect {
        let Some(prompt) = self.prompt.as_mut() else {
            return Effect::None;
        };
        self.notice = None;
        match key {
            "ctrl+c" => return Effect::Quit,
            "esc" => self.prompt = None,
            "backspace" => {
                prompt.text.pop();
            }
            "enter" => {
                let text = prompt.text.trim().to_string();
                if text.is_empty() {
                    self.notice = Some("an answer needs words".into());
                    return Effect::Redraw;
                }
                let (ask, question) = (prompt.ask.clone(), prompt.question.clone());
                match src.answer(&ask, &text) {
                    Ok(()) => {
                        self.notice = Some(format!("answered · {question}"));
                        self.prompt = None;
                        self.detail = false;
                    }
                    Err(e) => self.notice = Some(format!("not recorded: {e}")),
                }
            }
            _ => match typed {
                Some(c) if !c.is_control() => prompt.text.push(c),
                _ => return Effect::None,
            },
        }
        Effect::Redraw
    }
}
