//! Composed rendering helpers built on comfy-table, painted in the palette.
//!
//! comfy-table strips its own color automatically when stdout is not a
//! terminal, so the same call yields a colored table for a human and plain
//! bordered text in a pipe — keeping the agent/`--json` paths clean.

use comfy_table::presets::UTF8_BORDERS_ONLY;
use comfy_table::{Cell, Color, ContentArrangement, Table};

use crate::theme::{self, Rgb};

fn to_ct(color: Rgb) -> Color {
    let Rgb(r, g, b) = color;
    Color::Rgb { r, g, b }
}

/// A table with clay headers and faint borders whose columns wrap and align to
/// the terminal width. `rows` are rendered in ink; headers in clay.
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut table = Table::new();
    table
        .load_preset(UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(
        headers
            .iter()
            .map(|header| Cell::new(header).fg(to_ct(theme::CLAY)))
            .collect::<Vec<_>>(),
    );
    for row in rows {
        table.add_row(row.iter().map(Cell::new).collect::<Vec<_>>());
    }
    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_a_bordered_table_with_all_cells() {
        let out = table(
            &["Kind", "Count"],
            &[
                vec!["episodes".to_string(), "9".to_string()],
                vec!["claims".to_string(), "4".to_string()],
            ],
        );
        // Borders are present and every supplied cell shows up.
        assert!(out.contains("Kind") && out.contains("Count"));
        assert!(out.contains("episodes") && out.contains("9"));
        assert!(out.contains("claims") && out.contains("4"));
        assert!(out.contains('\u{2500}')); // box-drawing border
    }
}
