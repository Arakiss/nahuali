//! The clay-on-coffee palette and the single decision of whether to colorize.

use std::io::IsTerminal;

/// A 24-bit terminal color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

// The palette is the same identity as the Nahuali HTML artifacts, so the
// terminal, the TUI, and the web dossiers read as one product.
/// Primary accent — signal, used sparingly (headings, the live cursor).
pub const CLAY: Rgb = Rgb(217, 119, 87);
/// Trustworthy / certify.
pub const GREEN: Rgb = Rgb(143, 184, 122);
/// Caution / warn.
pub const AMBER: Rgb = Rgb(224, 177, 94);
/// Conflict / block.
pub const RED: Rgb = Rgb(217, 112, 112);
/// Informational / advisory.
pub const BLUE: Rgb = Rgb(127, 168, 201);
/// Primary text.
pub const INK: Rgb = Rgb(240, 238, 230);
/// Secondary text.
pub const INK_DIM: Rgb = Rgb(185, 177, 166);
/// Tertiary text — ids, counts, rules.
pub const INK_FAINT: Rgb = Rgb(131, 123, 113);
/// A raised coffee surface — the selection bar / highlighted row background.
pub const SURFACE: Rgb = Rgb(42, 36, 32);

/// Whether styled (colored) output should be emitted on stdout right now.
///
/// False when `NO_COLOR` is set or stdout is not a terminal, so piped,
/// redirected, and `--json` output stays plain for agents, hooks, and scripts.
pub fn colors_enabled() -> bool {
    std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal()
}
