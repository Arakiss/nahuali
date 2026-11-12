//! The frozen v0 view model (`docs/contracts.md`, "View model").
//!
//! A [`Screen`] is a renderer-agnostic tree. The portable terminal renderer
//! (`nahuali-tui`) paints it with cells; a later Tern adapter can map every
//! node 1:1 onto Tern Surface Protocol elements. Nothing here knows about
//! terminals, colours or widths: tones are semantic and layout is decided by
//! the renderer.
//!
//! The contract fixes the names and the shape of [`Screen`], [`Node`],
//! [`Tone`] and [`MascotState`]. The helper types it references without
//! spelling out ([`Span`], [`Item`], [`Seg`], [`Icon`], [`KeyHint`]) are
//! defined here.

/// Semantic colour role. Renderers map each tone to their own palette.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tone {
    #[default]
    Neutral,
    Accent,
    Ok,
    Warn,
    Danger,
    Muted,
}

/// The axolotl's expression. Every state is triggered by a real signal of the
/// mind (design §10 bis) and is shown with a one-line caption explaining why.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum MascotState {
    #[default]
    Idle,
    Greeting,
    Listening,
    Thinking,
    Writing,
    Dreaming,
    Worried,
    Alarmed,
    Proud,
    Happy,
}

impl MascotState {
    /// Every state, in declaration order.
    pub const ALL: [Self; 10] = [
        Self::Idle,
        Self::Greeting,
        Self::Listening,
        Self::Thinking,
        Self::Writing,
        Self::Dreaming,
        Self::Worried,
        Self::Alarmed,
        Self::Proud,
        Self::Happy,
    ];

    /// Lower-case name, used in captions and as the asset directory name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Greeting => "greeting",
            Self::Listening => "listening",
            Self::Thinking => "thinking",
            Self::Writing => "writing",
            Self::Dreaming => "dreaming",
            Self::Worried => "worried",
            Self::Alarmed => "alarmed",
            Self::Proud => "proud",
            Self::Happy => "happy",
        }
    }

    /// Parse the lower-case name back into a state.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.name() == name)
    }

    /// The compact text face used where the sprite does not fit.
    pub fn face(self) -> &'static str {
        match self {
            Self::Idle | Self::Greeting | Self::Happy => "•ᴗ•",
            Self::Listening => "•o•",
            Self::Thinking => "•…•",
            Self::Writing => "•ᴗ•",
            Self::Dreaming => "-ᴗ-",
            Self::Worried => "•△•",
            Self::Alarmed => "◉□◉",
            Self::Proud => "^ᴗ^",
        }
    }

    /// Arbitration priority (design §10 bis): alarmed beats everything,
    /// worried beats the transient activity states, and the calm states lose
    /// to any signal.
    pub fn priority(self) -> u8 {
        match self {
            Self::Alarmed => 100,
            Self::Worried => 60,
            Self::Thinking | Self::Listening | Self::Writing => 40,
            Self::Greeting => 35,
            Self::Dreaming => 30,
            Self::Proud => 20,
            Self::Happy => 10,
            Self::Idle => 0,
        }
    }

    /// Activity states last only while the activity runs.
    pub fn is_transient(self) -> bool {
        matches!(self, Self::Greeting | Self::Listening | Self::Thinking | Self::Writing)
    }

    /// The next state in declaration order, wrapping (used by the demo key).
    pub fn next(self) -> Self {
        let index = Self::ALL.iter().position(|s| *s == self).unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

/// A run of text with one tone. `text` may contain `\n` to break the line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub tone: Tone,
    pub strong: bool,
}

impl Span {
    pub fn new(text: impl Into<String>, tone: Tone) -> Self {
        Self { text: text.into(), tone, strong: false }
    }

    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Neutral)
    }

    pub fn muted(text: impl Into<String>) -> Self {
        Self::new(text, Tone::Muted)
    }

    pub fn strong(text: impl Into<String>, tone: Tone) -> Self {
        Self { text: text.into(), tone, strong: true }
    }
}

/// One row of a [`Node::List`]: an optional leading glyph or score, the main
/// text, right-aligned metadata and an optional second line.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Item {
    pub lead: Option<Span>,
    pub text: Vec<Span>,
    pub meta: Vec<Span>,
    pub sub: Vec<Span>,
}

impl Item {
    pub fn new(text: Vec<Span>) -> Self {
        Self { text, ..Self::default() }
    }

    pub fn with_lead(mut self, lead: Span) -> Self {
        self.lead = Some(lead);
        self
    }

    pub fn with_meta(mut self, meta: Vec<Span>) -> Self {
        self.meta = meta;
        self
    }

    pub fn with_sub(mut self, sub: Vec<Span>) -> Self {
        self.sub = sub;
        self
    }
}

/// The semantic tree a renderer lays out.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Col(Vec<Node>),
    Row(Vec<Node>),
    Card {
        title: String,
        tone: Tone,
        body: Box<Node>,
    },
    List {
        items: Vec<Item>,
        selected: Option<usize>,
    },
    Text(Vec<Span>),
    Badge {
        text: String,
        tone: Tone,
    },
    /// `value` is a fraction in `0.0..=1.0`.
    Meter {
        label: String,
        value: f32,
    },
    Kv(Vec<(String, String)>),
    Mascot {
        state: MascotState,
        caption: String,
    },
    Empty {
        title: String,
        line: String,
    },
}

impl Node {
    pub fn card(title: impl Into<String>, tone: Tone, body: Node) -> Self {
        Self::Card { title: title.into(), tone, body: Box::new(body) }
    }

    /// The first list that carries a selection, depth first.
    pub fn selectable_list(&self) -> Option<(&[Item], usize)> {
        match self {
            Self::List { items, selected: Some(selected) } => Some((items, *selected)),
            Self::Col(children) | Self::Row(children) => children.iter().find_map(Self::selectable_list),
            Self::Card { body, .. } => body.selectable_list(),
            _ => None,
        }
    }

    /// The first mascot in the tree, depth first.
    pub fn mascot(&self) -> Option<MascotState> {
        match self {
            Self::Mascot { state, .. } => Some(*state),
            Self::Col(children) | Self::Row(children) => children.iter().find_map(Self::mascot),
            Self::Card { body, .. } => body.mascot(),
            _ => None,
        }
    }
}

/// A status-line glyph. The renderer picks the character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Ok,
    Fail,
    Live,
    Idle,
    Mascot(MascotState),
}

/// One status-line segment. When the line is too narrow, segments drop from
/// the highest `priority` number down; priority 0 is pinned and only
/// truncated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seg {
    pub icon: Option<Icon>,
    pub text: String,
    pub tone: Tone,
    pub priority: u8,
}

impl Seg {
    pub fn new(icon: Option<Icon>, text: impl Into<String>, tone: Tone, priority: u8) -> Self {
        Self { icon, text: text.into(), tone, priority }
    }
}

/// One footer hint, generated from the keymap that handles the keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyHint {
    pub key: String,
    pub label: String,
}

/// A whole screen: chrome plus body.
#[derive(Clone, Debug, PartialEq)]
pub struct Screen {
    pub title: String,
    /// Who the session is for, as the header names it.
    pub who: String,
    pub tabs: Vec<String>,
    pub active_tab: usize,
    pub body: Node,
    pub status: Vec<Seg>,
    pub keys: Vec<KeyHint>,
}

impl Screen {
    /// The mascot state the screen shows, in the body or in the status line.
    pub fn mascot(&self) -> Option<MascotState> {
        self.body.mascot().or_else(|| {
            self.status.iter().find_map(|seg| match seg.icon {
                Some(Icon::Mascot(state)) => Some(state),
                _ => None,
            })
        })
    }
}
