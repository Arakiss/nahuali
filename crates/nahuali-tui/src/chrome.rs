//! Screen chrome: the tab bar on top, the status and key-hint row at the
//! bottom, the help overlay, and [`draw_screen`] that puts it all together.

use nahuali_view::{Icon, KeyHint, MascotState, Node, Screen, Seg, Tone};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::render::{Density, Painter, SpritePainter};
use crate::text::{self, Run};
use crate::theme::{Theme, raw};

/// Layout class for a terminal size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// ≥ 100×30: the sketch layout.
    Roomy,
    /// Anything smaller: the 80×24 wireframes.
    Narrow,
}

impl Fit {
    pub fn of(area: Rect) -> Self {
        if area.width >= 100 && area.height >= 30 { Self::Roomy } else { Self::Narrow }
    }
}

/// Paint a whole screen into `buf`.
pub fn draw_screen(
    screen: &Screen,
    area: Rect,
    buf: &mut Buffer,
    theme: &Theme,
    sprites: &mut dyn SpritePainter,
    help: Option<&Node>,
) {
    text::fill(buf, area, theme.base());
    if area.width < 20 || area.height < 8 {
        text::put(buf, area.x, area.y, "nahuali: window too small", theme.faint(), area.right());
        return;
    }
    let fit = Fit::of(area);
    let header_h = if fit == Fit::Roomy { 4 } else { 2 };
    let footer_h = 2;
    header(screen, Rect::new(area.x, area.y, area.width, header_h), buf, theme, fit);
    let footer_area = Rect::new(area.x, area.bottom() - footer_h, area.width, footer_h);
    footer(screen, footer_area, buf, theme);

    let body_top = area.y + header_h;
    let body = Rect::new(
        area.x + 1,
        body_top,
        area.width.saturating_sub(2),
        footer_area.y.saturating_sub(body_top + u16::from(fit == Fit::Roomy)),
    );
    // Try with air first; fall back to packed when it does not fit.
    let roomy = fit == Fit::Roomy;
    let mut density = Density { roomy, airy: roomy };
    {
        let probe = Painter { theme, density, sprites: &mut *sprites };
        if probe.natural_height(&screen.body, body.width) > body.height {
            density.airy = false;
        }
    }
    let mut painter = Painter { theme, density, sprites };
    painter.node(&screen.body, body, buf);

    if let Some(help) = help {
        overlay(help, area, buf, theme, &mut painter);
    }
}

fn header(screen: &Screen, area: Rect, buf: &mut Buffer, theme: &Theme, fit: Fit) {
    let brand = Style::default().fg(theme.color(raw::ACCENT)).add_modifier(Modifier::BOLD);
    let label_row = if fit == Fit::Roomy { area.y + 1 } else { area.y };
    let mut x = area.x + 2;
    x = text::put(buf, x, label_row, "nahuali", brand, area.right());
    x += 3;
    let compact_tabs = fit == Fit::Narrow;
    // What the loop below advances by: compact tabs take their width plus 3
    // after a 2-cell lead; roomy ones keep the original allowance.
    let tabs_width: u16 = if compact_tabs {
        screen.tabs.iter().map(|t| text::width(t) + 3).sum()
    } else {
        screen.tabs.iter().map(|t| text::width(t) + 4 + 1).sum()
    };
    if area.width < x + 2 + tabs_width {
        // Too narrow for the tab bar: name the active view only.
        let active = screen.tabs.get(screen.active_tab).map_or("", String::as_str);
        text::put(
            buf,
            x - 2,
            label_row,
            &format!("· {active}"),
            theme.text(Tone::Neutral, true),
            area.right(),
        );
        let hint = format!("tab ▸ {}/{}", screen.active_tab + 1, screen.tabs.len());
        let w = text::width(&hint);
        text::put(buf, area.right().saturating_sub(w + 2), label_row, &hint, theme.faint(), area.right());
    } else {
        text::put(buf, x, label_row, "│", theme.faint(), area.right());
        x += if compact_tabs { 2 } else { 4 };
        for (i, tab) in screen.tabs.iter().enumerate() {
            let w = text::width(tab);
            let active = i == screen.active_tab;
            if compact_tabs {
                let style = if active {
                    Style::default()
                        .fg(theme.color(raw::ACCENT))
                        .bg(theme.selection())
                        .add_modifier(Modifier::BOLD)
                } else {
                    theme.text(Tone::Neutral, false)
                };
                text::put(buf, x, label_row, &format!(" {tab} "), style, area.right());
                x += w + 3;
            } else {
                if active {
                    let accent = Style::default().fg(theme.color(raw::ACCENT));
                    let line = "─".repeat(usize::from(w + 2));
                    text::put(buf, x, area.y, &format!("╭{line}╮"), accent, area.right());
                    text::put(buf, x, label_row, "│", accent, area.right());
                    text::put(buf, x + w + 3, label_row, "│", accent, area.right());
                    text::put(buf, x, area.y + 2, &format!("╰{line}╯"), accent, area.right());
                    text::put(buf, x + 2, label_row, tab, accent.add_modifier(Modifier::BOLD), area.right());
                } else {
                    text::put(buf, x + 2, label_row, tab, theme.text(Tone::Neutral, false), area.right());
                }
                x += w + 4 + 3;
            }
        }
        // Whose mind this is, at the right end when it fits.
        let who = text::width(&screen.who);
        let at = area.right().saturating_sub(who + 2);
        if who > 0 && at > x + 1 {
            text::put(buf, at, label_row, &screen.who, theme.faint(), area.right());
        }
    }
    let rule_y = area.bottom() - 1;
    let rule = "─".repeat(usize::from(area.width.saturating_sub(2)));
    text::put(buf, area.x + 1, rule_y, &rule, theme.rule(), area.right());
}

/// A footer element with its drop priority (higher drops first).
struct Piece {
    runs: Vec<Run>,
    priority: u8,
    pinned: bool,
}

fn icon_runs(icon: Icon, theme: &Theme) -> Vec<Run> {
    let single = |glyph: &str, rgb| {
        vec![Run { text: glyph.to_string(), style: Style::default().fg(theme.color(rgb)) }]
    };
    match icon {
        Icon::Ok => single("✓", raw::OK),
        Icon::Fail => single("✗", raw::DANGER),
        Icon::Live => single("●", raw::OK),
        Icon::Idle => single("○", raw::INK_FAINT),
        Icon::Mascot(state) => mascot_mark(state, theme),
    }
}

/// The compact axolotl face: `≋(•△•)≋`, gills in coral.
pub fn mascot_mark(state: MascotState, theme: &Theme) -> Vec<Run> {
    let gill = Style::default().fg(theme.color(raw::GILL)).add_modifier(Modifier::BOLD);
    let body = Style::default().fg(theme.color(raw::INK_DIM));
    let face_tone = match state {
        MascotState::Alarmed => raw::DANGER,
        MascotState::Worried => raw::WARN,
        _ => raw::INK,
    };
    let face = Style::default().fg(theme.color(face_tone)).add_modifier(Modifier::BOLD);
    vec![
        Run { text: "≋".into(), style: gill },
        Run { text: "(".into(), style: body },
        Run { text: state.face().into(), style: face },
        Run { text: ")".into(), style: body },
        Run { text: "≋".into(), style: gill },
    ]
}

fn seg_piece(seg: &Seg, theme: &Theme) -> Piece {
    let mut runs = seg.icon.map(|i| icon_runs(i, theme)).unwrap_or_default();
    if !runs.is_empty() {
        runs.push(Run { text: " ".into(), style: Style::default() });
    }
    runs.push(Run { text: seg.text.clone(), style: theme.text(seg.tone, false) });
    Piece { runs, priority: seg.priority, pinned: seg.priority == 0 }
}

/// Drop order for hints: global shortcuts go first, the view's own keys
/// and `?` stay longest (help lists everything that was dropped).
fn hint_priority(hint: &KeyHint) -> u8 {
    match hint.key.as_str() {
        "?" => 1,
        "q" | "esc" => 3,
        "/" => 5,
        "1-6" => 6,
        _ => 2,
    }
}

fn hint_piece(hint: &KeyHint, theme: &Theme) -> Piece {
    Piece {
        runs: vec![
            Run { text: hint.key.clone(), style: Style::default().fg(theme.color(raw::ACCENT_SOFT)) },
            Run { text: format!(" {}", hint.label), style: theme.tone(Tone::Muted) },
        ],
        priority: hint_priority(hint),
        pinned: false,
    }
}

fn pieces_width(pieces: &[Piece], gap: u16) -> u16 {
    let content: u16 = pieces.iter().map(|p| p.runs.iter().map(|r| text::width(&r.text)).sum::<u16>()).sum();
    content + gap * pieces.len().saturating_sub(1) as u16
}

/// Which status segments and key hints survive at `width` cells.
/// Returns indices into `screen.status` and `screen.keys`.
pub fn footer_fit(screen: &Screen, width: u16, theme: &Theme) -> (Vec<usize>, Vec<usize>) {
    let segs: Vec<Piece> = screen.status.iter().map(|s| seg_piece(s, theme)).collect();
    let hints: Vec<Piece> = screen.keys.iter().map(|h| hint_piece(h, theme)).collect();
    let mut keep_segs: Vec<bool> = vec![true; segs.len()];
    let mut keep_hints: Vec<bool> = vec![true; hints.len()];
    const GAP: u16 = 3;
    let total = |ks: &[bool], kh: &[bool]| {
        let s: Vec<&Piece> = segs.iter().zip(ks).filter(|(_, k)| **k).map(|(p, _)| p).collect();
        let h: Vec<&Piece> = hints.iter().zip(kh).filter(|(_, k)| **k).map(|(p, _)| p).collect();
        let sw = pieces_width_refs(&s, GAP);
        let hw = pieces_width_refs(&h, GAP);
        sw + hw + if sw > 0 && hw > 0 { 4 } else { 0 }
    };
    let budget = width.saturating_sub(4);
    while total(&keep_segs, &keep_hints) > budget {
        // Drop the highest priority number; segments go before hints on ties.
        let hint = (0..hints.len()).filter(|i| keep_hints[*i]).max_by_key(|i| (hints[*i].priority, *i));
        let seg = (0..segs.len())
            .filter(|i| keep_segs[*i] && !segs[*i].pinned)
            .max_by_key(|i| (segs[*i].priority, *i));
        match (hint, seg) {
            (Some(h), Some(s)) if hints[h].priority > segs[s].priority => keep_hints[h] = false,
            (_, Some(s)) => keep_segs[s] = false,
            (Some(h), None) => keep_hints[h] = false,
            (None, None) => break,
        }
    }
    (
        (0..segs.len()).filter(|i| keep_segs[*i]).collect(),
        (0..hints.len()).filter(|i| keep_hints[*i]).collect(),
    )
}

fn pieces_width_refs(pieces: &[&Piece], gap: u16) -> u16 {
    let content: u16 = pieces.iter().map(|p| p.runs.iter().map(|r| text::width(&r.text)).sum::<u16>()).sum();
    content + gap * pieces.len().saturating_sub(1) as u16
}

fn footer(screen: &Screen, area: Rect, buf: &mut Buffer, theme: &Theme) {
    let rule = "─".repeat(usize::from(area.width.saturating_sub(2)));
    text::put(buf, area.x + 1, area.y, &rule, theme.rule(), area.right());
    let y = area.y + 1;
    let (segs, hints) = footer_fit(screen, area.width, theme);
    let right_edge = area.right().saturating_sub(2);
    let hint_pieces: Vec<Piece> = hints.iter().map(|i| hint_piece(&screen.keys[*i], theme)).collect();
    let hints_w = pieces_width(&hint_pieces, 3);
    let hints_x = right_edge.saturating_sub(hints_w);
    let seg_limit = if hint_pieces.is_empty() { right_edge } else { hints_x.saturating_sub(4) };
    let mut x = area.x + 2;
    for (n, i) in segs.iter().enumerate() {
        if n > 0 {
            x += 3;
        }
        x = text::put_runs(buf, x, y, seg_limit, &seg_piece(&screen.status[*i], theme).runs);
    }
    let mut x = hints_x;
    for (n, piece) in hint_pieces.iter().enumerate() {
        if n > 0 {
            x += 3;
        }
        x = text::put_runs(buf, x, y, right_edge, &piece.runs);
    }
}

/// Mix a colour two thirds of the way towards the background.
fn dim(theme: &Theme, color: ratatui::style::Color) -> ratatui::style::Color {
    use ratatui::style::Color;
    let bg = raw::BG;
    let mix = |c: u8, b: u8| ((u16::from(c) + 2 * u16::from(b)) / 3) as u8;
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(mix(r, bg.0), mix(g, bg.1), mix(b, bg.2)),
        Color::Reset => color,
        _ => theme.color(raw::BORDER_DIM),
    }
}

fn overlay(help: &Node, area: Rect, buf: &mut Buffer, theme: &Theme, painter: &mut Painter<'_>) {
    let width = 56.min(area.width.saturating_sub(4));
    let saved = painter.density;
    painter.density = Density { roomy: false, airy: false };
    let height = painter.natural_height(help, width).min(area.height.saturating_sub(4));
    let rect =
        Rect::new(area.x + (area.width - width) / 2, area.y + (area.height - height) / 2, width, height);
    // Dim everything behind the card so the keys are the only thing to read.
    // Kitty placeholders are skipped: their colour is the image id.
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                if cell.symbol().starts_with('\u{10EEEE}') {
                    continue;
                }
                cell.fg = dim(theme, cell.fg);
                cell.bg = dim(theme, cell.bg);
                cell.modifier.remove(Modifier::BOLD);
            }
        }
    }
    text::fill(buf, rect, theme.base());
    painter.sprites.occlude(rect);
    painter.node(help, rect, buf);
    painter.density = saved;
}
