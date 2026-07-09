//! `nahuali config` — print the resolved effective configuration.
//!
//! This is the one place an operator can see exactly which endpoint, namespace,
//! database, and archive database a command will use, and whether each value
//! came from a flag, an environment variable, or the built-in default. It never
//! opens the store and never prints credentials.

use nahuali_core::{ResolvedValue, resolve_config};

use crate::output;

/// Resolve and print the effective configuration for the given `--database` flag.
pub(crate) fn config(database_flag: Option<&str>, json: bool) -> anyhow::Result<()> {
    let resolved = resolve_config(database_flag)?;
    if json {
        output::print_json(&resolved)?;
    } else {
        println!("Nahuali effective configuration");
        println!();
        print_row("Endpoint", &resolved.endpoint);
        print_row("Namespace", &resolved.namespace);
        print_row("Database", &resolved.database);
        print_row("Archive database", &resolved.archive_database);
        println!();
        println!(
            "Sources: flag = --database; env = NAHUALI_DB_URL / NAHUALI_DB_NAMESPACE / \
NAHUALI_DB_DATABASE / NAHUALI_ARCHIVE_DB; default = built-in."
        );
        println!(
            "Credentials are never shown (NAHUALI_DB_USERNAME / NAHUALI_DB_PASSWORD are not printed)."
        );
    }
    Ok(())
}

fn print_row(label: &str, value: &ResolvedValue) {
    println!(
        "  {:<18}{:<28}({})",
        label,
        value.value,
        value.source.as_str()
    );
}
