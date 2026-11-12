//! Lays out and paints a [`Node`] tree into a ratatui buffer.
//!
//! The view model carries no sizes, so layout follows a few rules:
//! - A `Row` whose first child is a `Col` starting with a `Mascot` is a
//!   sidebar layout: a fixed column with a divider when roomy, a band on top
//!   when narrow.
//! - Other `Row`s give children their natural width when it is under a third
//!   of the row, share the rest equally and stretch every child to the row's
//!   height. On a narrow screen a row whose children would get fewer than
//!   30 cells stacks into a column.
//! - A `Col` gives children their natural height; spare rows go to children
//!   that contain a `Row`, missing rows are taken from the tallest child.
//! - Roomy screens (≥100×30) put card titles inside the card and leave air
//!   between blocks; narrow ones put titles on the border and pack tighter.

use nahuali_view::{Item, MascotState, Node, Span, Tone};
use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::{Modifier, Style};

use crate::text::{self, Run};
use crate::theme::{Theme, raw};

/// Paints the axolotl. Implemented by [`crate::mascot::Sprites`].
pub trait SpritePainter {
    /// The sprite size (in cells) that fits inside `max`.
    fn fit(&self, max: Size) -> Size;
    /// Paint the current frame, horizontally centred, at the top of `area`.
    fn paint(&mut self, area: Rect, buf: &mut Buffer);
    /// `rect` was painted over what is already drawn. A sprite that lives
    /// outside the cell grid has to give way; one made of cells is covered.
    fn occlude(&mut self, _rect: Rect) {}
}

/// A painter that draws nothing (layout tests).
pub struct NoSprites;

impl SpritePainter for NoSprites {
    fn fit(&self, max: Size) -> Size {
        Size::new(max.width.min(max.height * 2), max.height.min(max.width / 2))
    }

    fn paint(&mut self, _area: Rect, _buf: &mut Buffer) {}
}

pub const SIDEBAR_WIDTH: u16 = 32;
const STACK_BELOW: u16 = 30;

/// How much air the layout can afford.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Density {
    /// Titles inside cards, gaps between blocks, sidebar as a column.
    pub roomy: bool,
    /// A blank row between list items.
    pub airy: bool,
}

pub struct Painter<'a> {
    pub theme: &'a Theme,
    pub density: Density,
    pub sprites: &'a mut dyn SpritePainter,
}

fn is_sidebar(node: &Node) -> bool {
    matches!(node, Node::Col(children) if matches!(children.first(), Some(Node::Mascot { .. })))
}

fn grows(node: &Node) -> bool {
    match node {
        Node::Row(_) => true,
        Node::Col(children) => children.iter().any(grows),
        _ => false,
    }
}

fn is_heading(node: &Node) -> bool {
    matches!(node, Node::Text(spans) if !spans.is_empty() && spans.iter().all(|s| s.strong))
}

fn same_leaf(a: &Node, b: &Node) -> bool {
    matches!((a, b), (Node::Meter { .. }, Node::Meter { .. }) | (Node::Badge { .. }, Node::Badge { .. }))
}

fn runs(theme: &Theme, spans: &[Span]) -> Vec<Run> {
    spans.iter().map(|s| Run { text: s.text.clone(), style: theme.text(s.tone, s.strong) }).collect()
}

fn spans_width(spans: &[Span]) -> u16 {
    spans.iter().map(|s| text::width(&s.text)).sum()
}

impl Painter<'_> {
    fn pad(&self) -> u16 {
        if self.density.roomy { 2 } else { 1 }
    }

    fn block_gap(&self) -> u16 {
        u16::from(self.density.roomy)
    }

    // -----------------------------------------------------------------
    // Measuring

    /// Preferred width, used to keep narrow panels narrow.
    pub fn natural_width(&self, node: &Node) -> u16 {
        match node {
            Node::Col(children) => children.iter().map(|c| self.natural_width(c)).max().unwrap_or(0),
            Node::Row(children) => {
                let gaps = children.len().saturating_sub(1) as u16;
                children.iter().map(|c| self.natural_width(c)).sum::<u16>() + gaps
            }
            Node::Card { title, body, .. } => {
                (self.natural_width(body).max(text::width(title))) + 2 + 2 * self.pad()
            }
            Node::List { items, .. } => {
                let lead = lead_width(items);
                items
                    .iter()
                    .map(|i| {
                        let meta = spans_width(&i.meta);
                        let gap = if meta > 0 { 2 } else { 0 };
                        let lead = if lead > 0 { lead + 2 } else { 0 };
                        let main = spans_width(&i.text).max(spans_width(&i.sub));
                        lead + main + gap + meta
                    })
                    .max()
                    .unwrap_or(0)
            }
            Node::Text(spans) => {
                let joined: String = spans.iter().map(|s| s.text.as_str()).collect();
                joined.split('\n').map(text::width).max().unwrap_or(0)
            }
            Node::Badge { text, .. } => text::width(text) + 2,
            Node::Meter { .. } => 40,
            Node::Kv(pairs) => {
                pairs.iter().map(|(k, v)| text::width(k) + 2 + text::width(v)).max().unwrap_or(0)
            }
            Node::Mascot { caption, .. } => text::width(caption).max(26),
            Node::Empty { title, line } => text::width(title).max(text::width(line)),
        }
    }

    /// Height wanted at `width`.
    pub fn natural_height(&self, node: &Node, width: u16) -> u16 {
        match node {
            Node::Col(children) => {
                let gaps: u16 = children.windows(2).map(|w| self.gap_between(&w[0], &w[1])).sum();
                children.iter().map(|c| self.natural_height(c, width)).sum::<u16>() + gaps
            }
            Node::Row(children) => {
                if self.sidebar_band(node) {
                    let band = self.band_height(&children[0], width);
                    let rest = Node::Col(children[1..].to_vec());
                    return band + self.natural_height(&rest, width);
                }
                let widths = self.row_widths(children, width);
                if self.should_stack(&widths) {
                    return self.natural_height(&Node::Col(children.clone()), width);
                }
                children.iter().zip(widths).map(|(c, w)| self.natural_height(c, w)).max().unwrap_or(0)
            }
            Node::Card { title, body, .. } => {
                let inner = width.saturating_sub(2 + 2 * self.pad());
                let title_rows = self.title_rows(title);
                2 + title_rows + self.natural_height(body, inner)
            }
            Node::List { items, .. } => {
                let rows: u16 = items.iter().map(|i| 1 + u16::from(!i.sub.is_empty())).sum();
                let gaps = if self.density.airy { items.len().saturating_sub(1) as u16 } else { 0 };
                rows + gaps
            }
            Node::Text(spans) => text::wrap(&runs(self.theme, spans), width).len() as u16,
            Node::Badge { .. } | Node::Meter { .. } => 1,
            Node::Kv(pairs) => pairs.len() as u16,
            Node::Mascot { caption, .. } => {
                if self.portrait(width) {
                    let sprite = self.sprites.fit(Size::new(width.min(26), 13));
                    let caption_rows =
                        text::wrap(&[Run { text: caption.clone(), style: Style::default() }], width).len()
                            as u16;
                    sprite.height + 1 + caption_rows
                } else if self.density.roomy {
                    7
                } else {
                    5
                }
            }
            Node::Empty { .. } => 2,
        }
    }

    fn title_rows(&self, title: &str) -> u16 {
        if title.is_empty() || !self.density.roomy { 0 } else { 1 + u16::from(self.density.airy) }
    }

    fn gap_between(&self, a: &Node, b: &Node) -> u16 {
        if is_heading(a) {
            return 0;
        }
        if same_leaf(a, b) {
            return u16::from(self.density.airy);
        }
        let layout = |n: &Node| matches!(n, Node::Card { .. } | Node::Row(_) | Node::Col(_));
        if layout(a) || layout(b) { self.block_gap() } else { 1 }
    }

    fn portrait(&self, width: u16) -> bool {
        width <= 40
    }

    fn sidebar_band(&self, node: &Node) -> bool {
        matches!(node, Node::Row(children) if !self.density.roomy && children.first().is_some_and(is_sidebar))
    }

    fn row_widths(&self, children: &[Node], width: u16) -> Vec<u16> {
        let n = children.len() as u16;
        if n == 0 {
            return Vec::new();
        }
        let gap = 1;
        let usable = width.saturating_sub(gap * (n - 1));
        let third = usable / 3;
        let naturals: Vec<u16> = children.iter().map(|c| self.natural_width(c)).collect();
        let fixed: Vec<bool> = naturals.iter().map(|w| n > 1 && *w <= third && *w > 0).collect();
        let fixed_total: u16 = naturals.iter().zip(&fixed).filter(|(_, f)| **f).map(|(w, _)| *w).sum();
        let rest = usable.saturating_sub(fixed_total);
        // Roomy rows split the rest equally (aligned grids look calm). Narrow
        // rows share it in proportion to natural width, so the wordier panel
        // truncates less, each kept within 80-125 % of an equal share.
        let flex = fixed.iter().filter(|f| !**f).count() as u32;
        let equal = u32::from(rest).checked_div(flex).unwrap_or(0);
        let flex_natural: u32 =
            naturals.iter().zip(&fixed).filter(|(_, f)| !**f).map(|(w, _)| u32::from(*w).max(1)).sum();
        let roomy = self.density.roomy;
        let mut widths: Vec<u16> = naturals
            .iter()
            .zip(&fixed)
            .map(|(w, f)| {
                if *f {
                    *w
                } else if roomy || flex_natural == 0 {
                    equal as u16
                } else {
                    let share = u32::from(rest) * u32::from((*w).max(1)) / flex_natural;
                    share.clamp(equal * 4 / 5, equal * 5 / 4) as u16
                }
            })
            .collect();
        // Give the remainder to the last flexible child so the row is flush.
        let used: u16 = widths.iter().sum();
        if let Some(i) = fixed.iter().rposition(|f| !*f) {
            widths[i] += usable.saturating_sub(used);
        }
        widths
    }

    fn should_stack(&self, widths: &[u16]) -> bool {
        !self.density.roomy && widths.len() > 1 && widths.iter().any(|w| *w < STACK_BELOW)
    }

    // -----------------------------------------------------------------
    // Painting

    pub fn node(&mut self, node: &Node, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        match node {
            Node::Col(children) => self.col(children, area, buf),
            Node::Row(children) => self.row(node, children, area, buf),
            Node::Card { title, tone, body } => self.card(title, *tone, body, area, buf),
            Node::List { items, selected } => self.list(items, *selected, area, buf),
            Node::Text(spans) => self.text(spans, area, buf),
            Node::Badge { text, tone } => self.badge(text, *tone, area, buf),
            Node::Meter { label, value } => self.meter(label, *value, area, buf),
            Node::Kv(pairs) => self.kv(pairs, area, buf),
            Node::Mascot { state, caption } => self.mascot(*state, caption, area, buf),
            Node::Empty { title, line } => self.empty(title, line, area, buf),
        }
    }

    fn col(&mut self, children: &[Node], area: Rect, buf: &mut Buffer) {
        let gaps: Vec<u16> = children.windows(2).map(|w| self.gap_between(&w[0], &w[1])).collect();
        let gap_total: u16 = gaps.iter().sum();
        let available = area.height.saturating_sub(gap_total);
        let mut heights: Vec<u16> = children.iter().map(|c| self.natural_height(c, area.width)).collect();
        let total: u16 = heights.iter().sum();
        if total < available {
            let growers: Vec<usize> = (0..children.len()).filter(|i| grows(&children[*i])).collect();
            if !growers.is_empty() {
                let spare = available - total;
                let each = spare / growers.len() as u16;
                for (k, i) in growers.iter().enumerate() {
                    heights[*i] += each;
                    if k == growers.len() - 1 {
                        heights[*i] += spare - each * growers.len() as u16;
                    }
                }
            }
        } else {
            self.shrink(children, &mut heights, &gaps, total - available, area.width);
        }
        let mut y = area.y;
        for (i, child) in children.iter().enumerate() {
            if heights[i] == 0 {
                continue;
            }
            let h = heights[i].min(area.bottom().saturating_sub(y));
            if h == 0 {
                break;
            }
            self.node(child, Rect::new(area.x, y, area.width, h), buf);
            y += h + gaps.get(i).copied().unwrap_or(0);
            if y >= area.bottom() {
                break;
            }
        }
    }

    /// Take `over` rows away, the way a designer would: later blocks yield
    /// first (down to four rows), then disappear, and only then does the
    /// first block shrink. Blocks holding the axolotl never shrink.
    fn shrink(&self, children: &[Node], heights: &mut [u16], gaps: &[u16], over: u16, width: u16) {
        let holds_mascot = |n: &Node| match n {
            Node::Mascot { .. } => true,
            Node::Card { body, .. } => matches!(body.as_ref(), Node::Mascot { .. }),
            _ => false,
        };
        let floor = |n: &Node| if matches!(n, Node::Card { .. }) { 3 } else { 1 };
        let mut over = over;
        let last = children.len().saturating_sub(1);
        for i in (1..=last).rev() {
            if over == 0 || holds_mascot(&children[i]) {
                continue;
            }
            let keep = 4.min(heights[i]);
            let take = (heights[i] - keep).min(over);
            heights[i] -= take;
            over -= take;
        }
        for i in (1..=last).rev() {
            if over == 0 {
                break;
            }
            if matches!(children[i], Node::Card { .. }) && !holds_mascot(&children[i]) {
                let freed = heights[i] + gaps.get(i - 1).copied().unwrap_or(0);
                heights[i] = 0;
                let back = freed.saturating_sub(over);
                over = over.saturating_sub(freed);
                // Rows freed beyond the need go back, first block first.
                let mut back = back;
                for j in 0..children.len() {
                    if back == 0 || heights[j] == 0 {
                        continue;
                    }
                    let want = self.natural_height(&children[j], width);
                    let give = want.saturating_sub(heights[j]).min(back);
                    heights[j] += give;
                    back -= give;
                }
            }
        }
        while over > 0 {
            let candidate = (0..children.len())
                .filter(|i| heights[*i] > floor(&children[*i]) && !holds_mascot(&children[*i]))
                .max_by_key(|i| (heights[*i], *i));
            let Some(i) = candidate else { break };
            heights[i] -= 1;
            over -= 1;
        }
    }

    fn row(&mut self, node: &Node, children: &[Node], area: Rect, buf: &mut Buffer) {
        if children.first().is_some_and(is_sidebar) {
            if self.sidebar_band(node) {
                let band = self.band_height(&children[0], area.width).min(area.height);
                self.band(&children[0], Rect::new(area.x, area.y, area.width, band), buf);
                let rest = Node::Col(children[1..].to_vec());
                let top = area.y + band;
                self.node(&rest, Rect::new(area.x, top, area.width, area.bottom().saturating_sub(top)), buf);
            } else {
                self.sidebar_columns(children, area, buf);
            }
            return;
        }
        let widths = self.row_widths(children, area.width);
        if self.should_stack(&widths) {
            self.col(children, area, buf);
            return;
        }
        let mut x = area.x;
        for (child, w) in children.iter().zip(widths) {
            let w = w.min(area.right().saturating_sub(x));
            self.node(child, Rect::new(x, area.y, w, area.height), buf);
            x += w + 1;
        }
    }

    // -- sidebar ------------------------------------------------------

    fn sidebar_columns(&mut self, children: &[Node], area: Rect, buf: &mut Buffer) {
        let width = SIDEBAR_WIDTH.min(area.width / 3);
        let side = Rect::new(area.x, area.y, width, area.height);
        self.sidebar(&children[0], side, buf);
        let divider_x = area.x + width;
        for y in area.top()..area.bottom() {
            text::put(buf, divider_x, y, "│", self.theme.divider(), divider_x + 1);
        }
        let x = divider_x + 3;
        let main = Rect::new(x, area.y + 1, area.right().saturating_sub(x), area.height.saturating_sub(1));
        let rest = Node::Col(children[1..].to_vec());
        self.node(&rest, main, buf);
    }

    fn sidebar(&mut self, node: &Node, area: Rect, buf: &mut Buffer) {
        let Node::Col(children) = node else { return };
        let inner =
            Rect::new(area.x + 1, area.y + 1, area.width.saturating_sub(2), area.height.saturating_sub(1));
        let mut y = inner.y;
        for child in children {
            if y >= inner.bottom() {
                break;
            }
            match child {
                Node::Mascot { state, caption } => {
                    let h = self.natural_height(child, inner.width).min(inner.bottom() - y);
                    self.mascot(*state, caption, Rect::new(inner.x, y, inner.width, h), buf);
                    y += h + 1;
                }
                Node::Card { title, body, .. } => {
                    let rule = "─".repeat(usize::from(inner.width));
                    text::put(buf, inner.x, y, &rule, self.theme.divider(), inner.right());
                    y += 2;
                    if y >= inner.bottom() {
                        break;
                    }
                    text::put(buf, inner.x, y, title, self.theme.title(Tone::Accent), inner.right());
                    y += 1 + u16::from(self.density.airy);
                    let h = self.natural_height(body, inner.width).min(inner.bottom().saturating_sub(y));
                    self.node(body, Rect::new(inner.x, y, inner.width, h), buf);
                    y += h + 1;
                }
                other => {
                    let h = self.natural_height(other, inner.width).min(inner.bottom() - y);
                    self.node(other, Rect::new(inner.x, y, inner.width, h), buf);
                    y += h + 1;
                }
            }
        }
    }

    fn band_lines(&self, node: &Node) -> Vec<Vec<Span>> {
        let Node::Col(children) = node else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        for child in children.iter().skip(1) {
            match child {
                Node::Card { body, .. } => {
                    if let Node::List { items, .. } = body.as_ref() {
                        let joined: Vec<String> =
                            items.iter().map(|i| i.text.iter().map(|s| s.text.as_str()).collect()).collect();
                        lines.push(vec![Span::muted(joined.join(" · "))]);
                    }
                }
                Node::Text(spans) => lines.push(spans.clone()),
                _ => {}
            }
        }
        lines
    }

    fn band_height(&self, _node: &Node, _width: u16) -> u16 {
        6
    }

    /// Narrow sidebar: sprite on the left, caption and summary on the right.
    fn band(&mut self, node: &Node, area: Rect, buf: &mut Buffer) {
        let Node::Col(children) = node else { return };
        let Some(Node::Mascot { state, caption }) = children.first() else {
            return;
        };
        let sprite = self.sprites.fit(Size::new(16, area.height));
        let sprite_area = Rect::new(area.x + 1, area.y, sprite.width, area.height);
        self.sprites.paint(sprite_area, buf);
        let x = area.x + 1 + sprite.width + 2;
        let right = area.right().saturating_sub(1);
        let mut y = area.y;
        let caption_runs = self.caption_runs(*state, caption);
        text::put_runs(buf, x, y, right, &caption_runs);
        y += 1;
        // One line per section, truncated: the band never grows.
        for line in self.band_lines(node) {
            if y >= area.bottom() {
                return;
            }
            text::put_runs(buf, x, y, right, &runs(self.theme, &line));
            y += 1;
        }
    }

    fn caption_runs(&self, state: MascotState, caption: &str) -> Vec<Run> {
        let (name, rest) = caption.split_once(" · ").unwrap_or((caption, ""));
        let tone = match state {
            MascotState::Alarmed => Tone::Danger,
            MascotState::Worried => Tone::Warn,
            _ => Tone::Neutral,
        };
        let mut out = vec![Run { text: name.to_string(), style: self.theme.text(tone, true) }];
        if !rest.is_empty() {
            out.push(Run { text: format!(" · {rest}"), style: self.theme.text(Tone::Neutral, false) });
        }
        out
    }

    // -- leaves -------------------------------------------------------

    fn mascot(&mut self, state: MascotState, caption: &str, area: Rect, buf: &mut Buffer) {
        if self.portrait(area.width) {
            let sprite =
                self.sprites.fit(Size::new(area.width.min(26), 13.min(area.height.saturating_sub(2))));
            self.sprites.paint(Rect::new(area.x, area.y, area.width, sprite.height), buf);
            let caption_runs = self.caption_runs(state, caption);
            let lines = text::wrap(&caption_runs, area.width);
            let first = area.y + sprite.height + 1;
            for (y, line) in (first..area.bottom()).zip(lines) {
                let w: u16 = line.iter().map(|r| text::width(&r.text)).sum();
                let x = area.x + area.width.saturating_sub(w) / 2;
                text::put_runs(buf, x, y, area.right(), &line);
            }
            return;
        }
        // Band: sprite, divider, caption centred vertically.
        let sprite = self.sprites.fit(Size::new(area.width / 3, area.height));
        let sprite_area = Rect::new(area.x + 1, area.y, sprite.width, area.height);
        self.sprites.paint(sprite_area, buf);
        let divider_x = area.x + 1 + sprite.width + 2;
        let top = area.y + u16::from(area.height > 4);
        let bottom = area.bottom().saturating_sub(u16::from(area.height > 4));
        for y in top..bottom {
            text::put(buf, divider_x, y, "│", self.theme.divider(), divider_x + 1);
        }
        let x = divider_x + 3;
        let y = area.y + area.height / 2;
        let caption_runs = self.caption_runs(state, caption);
        text::put_runs(
            buf,
            x,
            y.saturating_sub(u16::from(area.height.is_multiple_of(2))),
            area.right(),
            &caption_runs,
        );
    }

    fn card(&mut self, title: &str, tone: Tone, body: &Node, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 4 {
            return;
        }
        let border = self.theme.border(tone);
        let (l, r, t, b) = (area.left(), area.right() - 1, area.top(), area.bottom() - 1);
        let horizontal = "─".repeat(usize::from(area.width.saturating_sub(2)));
        text::put(buf, l, t, "╭", border, r + 1);
        text::put(buf, l + 1, t, &horizontal, border, r);
        text::put(buf, r, t, "╮", border, r + 1);
        text::put(buf, l, b, "╰", border, r + 1);
        text::put(buf, l + 1, b, &horizontal, border, r);
        text::put(buf, r, b, "╯", border, r + 1);
        for y in (t + 1)..b {
            text::put(buf, l, y, "│", border, l + 1);
            text::put(buf, r, y, "│", border, r + 1);
        }
        let pad = self.pad();
        let mut inner = Rect::new(
            l + 1 + pad,
            t + 1,
            area.width.saturating_sub(2 + 2 * pad),
            area.height.saturating_sub(2),
        );
        if !title.is_empty() {
            if self.density.roomy {
                text::put(buf, inner.x, inner.y, title, self.theme.title(tone), inner.right());
                let skip = self.title_rows(title).min(inner.height);
                inner.y += skip;
                inner.height -= skip;
            } else {
                let label = format!(" {} ", text::truncate(title, area.width.saturating_sub(6)));
                text::put(buf, l + 2, t, &label, self.theme.title(tone), r);
            }
        }
        self.node(body, inner, buf);
    }

    fn list(&mut self, items: &[Item], selected: Option<usize>, area: Rect, buf: &mut Buffer) {
        if items.is_empty() {
            return;
        }
        let lead_w = lead_width(items);
        // Pick the most generous presentation that fits.
        let with_subs: u16 = items.iter().map(|i| 1 + u16::from(!i.sub.is_empty())).sum();
        let gaps = items.len().saturating_sub(1) as u16;
        let airy = self.density.airy && with_subs + gaps <= area.height;
        let subs = with_subs <= area.height || airy;
        let row_h = |i: &Item| 1 + u16::from(subs && !i.sub.is_empty());
        // Scroll so the selection stays visible.
        let mut first = 0;
        if let Some(sel) = selected {
            loop {
                let used: u16 =
                    items[first..=sel.min(items.len() - 1)].iter().map(|i| row_h(i) + u16::from(airy)).sum();
                if used <= area.height + u16::from(airy) || first >= sel {
                    break;
                }
                first += 1;
            }
        }
        let mut y = area.y;
        for (index, item) in items.iter().enumerate().skip(first) {
            let h = row_h(item);
            let hidden = items.len() - index;
            let is_selected = selected == Some(index);
            let no_room_after = y + h + u16::from(airy) >= area.bottom();
            if y + h > area.bottom() || (hidden > 1 && no_room_after && !is_selected) {
                // Say how many rows did not fit instead of cutting silently.
                if y < area.bottom() {
                    let more = format!("+{hidden} more");
                    text::put(buf, area.x, y, &more, self.theme.faint(), area.right());
                }
                break;
            }
            if is_selected {
                let bar = Rect::new(
                    area.x.saturating_sub(1),
                    y,
                    (area.width + 2).min(buf.area.right().saturating_sub(area.x.saturating_sub(1))),
                    h,
                );
                text::tint(buf, bar, self.theme.selection());
            }
            self.item(item, lead_w, is_selected, subs, Rect::new(area.x, y, area.width, h), buf);
            y += h + u16::from(airy);
        }
    }

    fn item(&mut self, item: &Item, lead_w: u16, selected: bool, subs: bool, area: Rect, buf: &mut Buffer) {
        let bg = selected.then(|| self.theme.selection());
        let style_of = |theme: &Theme, s: &Span| {
            let mut style = theme.text(s.tone, s.strong || (selected && s.tone == Tone::Neutral));
            if let Some(bg) = bg {
                style = style.bg(bg);
            }
            style
        };
        let mut x = area.x;
        if lead_w > 0 {
            if let Some(lead) = &item.lead {
                let mut style = style_of(self.theme, lead);
                if selected {
                    style = style.fg(self.theme.color(raw::ACCENT)).add_modifier(Modifier::BOLD);
                }
                text::put(buf, x, area.y, &lead.text, style, area.right());
            }
            x += lead_w + 2;
        }
        let meta: Vec<Run> =
            item.meta.iter().map(|s| Run { text: s.text.clone(), style: style_of(self.theme, s) }).collect();
        let meta_w: u16 = meta.iter().map(|r| text::width(&r.text)).sum();
        let text_right = if meta_w > 0 { area.right().saturating_sub(meta_w + 2) } else { area.right() };
        let main: Vec<Run> =
            item.text.iter().map(|s| Run { text: s.text.clone(), style: style_of(self.theme, s) }).collect();
        text::put_runs(buf, x, area.y, text_right, &main);
        if meta_w > 0 && area.width > meta_w + 4 {
            text::put_runs(buf, area.right() - meta_w, area.y, area.right(), &meta);
        }
        if subs && !item.sub.is_empty() && area.height > 1 {
            let sub: Vec<Run> = item
                .sub
                .iter()
                .map(|s| Run { text: s.text.clone(), style: style_of(self.theme, s) })
                .collect();
            text::put_runs(buf, x, area.y + 1, area.right(), &sub);
        }
    }

    fn text(&mut self, spans: &[Span], area: Rect, buf: &mut Buffer) {
        let lines = text::wrap(&runs(self.theme, spans), area.width);
        for (i, line) in lines.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            text::put_runs(buf, area.x, y, area.right(), line);
        }
    }

    fn badge(&mut self, label: &str, tone: Tone, area: Rect, buf: &mut Buffer) {
        let style = self.theme.text(tone, true).bg(self.theme.surface_soft());
        text::put(buf, area.x, area.y, &format!(" {label} "), style, area.right());
    }

    fn meter(&mut self, label: &str, value: f32, area: Rect, buf: &mut Buffer) {
        let value = value.clamp(0.0, 1.0);
        let label_w = 10.min(area.width / 3);
        let number = format!("{value:.2}");
        text::put(
            buf,
            area.x,
            area.y,
            &text::truncate(label, label_w),
            self.theme.text(Tone::Neutral, false),
            area.x + label_w,
        );
        let bar_x = area.x + label_w + 1;
        let bar_w = area.width.saturating_sub(label_w + 1 + 2 + text::width(&number));
        let filled = (value * f32::from(bar_w)).round() as u16;
        let fill = "━".repeat(usize::from(filled));
        let track = "━".repeat(usize::from(bar_w - filled));
        text::put(
            buf,
            bar_x,
            area.y,
            &fill,
            Style::default().fg(self.theme.color(raw::ACCENT_SOFT)),
            bar_x + bar_w,
        );
        text::put(
            buf,
            bar_x + filled,
            area.y,
            &track,
            Style::default().fg(self.theme.color(raw::BORDER_DIM)),
            bar_x + bar_w,
        );
        text::put(
            buf,
            area.right() - text::width(&number),
            area.y,
            &number,
            self.theme.text(Tone::Neutral, false),
            area.right(),
        );
    }

    fn kv(&mut self, pairs: &[(String, String)], area: Rect, buf: &mut Buffer) {
        let key_w = pairs.iter().map(|(k, _)| text::width(k)).max().unwrap_or(0).min(14).min(area.width / 3);
        for (i, (k, v)) in pairs.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            text::put(
                buf,
                area.x,
                y,
                &text::truncate(k, key_w),
                self.theme.tone(Tone::Muted),
                area.x + key_w,
            );
            let x = area.x + key_w + 2;
            text::put(
                buf,
                x,
                y,
                &text::truncate(v, area.right().saturating_sub(x)),
                self.theme.tone(Tone::Neutral),
                area.right(),
            );
        }
    }

    fn empty(&mut self, title: &str, line: &str, area: Rect, buf: &mut Buffer) {
        text::put(buf, area.x, area.y, title, self.theme.text(Tone::Muted, true), area.right());
        if area.height > 1 {
            let lines = text::wrap(&[Run { text: line.to_string(), style: self.theme.faint() }], area.width);
            if let Some(first) = lines.first() {
                text::put_runs(buf, area.x, area.y + 1, area.right(), first);
            }
        }
    }
}

fn lead_width(items: &[Item]) -> u16 {
    items.iter().filter_map(|i| i.lead.as_ref()).map(|l| text::width(&l.text)).max().unwrap_or(0)
}
