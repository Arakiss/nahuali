mod cli;
mod commands;
mod output;
mod style;
mod text_intake;

use std::ffi::OsString;
use std::io::IsTerminal;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if requests_version(&args) {
        print_version();
        return Ok(());
    }
    commands::run(cli::Cli::parse())
}

fn requests_version(args: &[OsString]) -> bool {
    for arg in args {
        if arg == "--" || arg == "--help" || arg == "-h" {
            return false;
        }
        if arg == "--version" || arg == "-V" {
            return true;
        }
    }
    false
}

fn print_version() {
    let version = env!("CARGO_PKG_VERSION");
    if std::io::stdout().is_terminal() {
        #[cfg(feature = "tui")]
        if nahuali_ui::version::render(version).is_ok() {
            return;
        }
    } else {
        println!("nahuali {version}");
        return;
    }
    println!("nahuali {version}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_version_before_clap_consumes_it() {
        assert!(requests_version(&[OsString::from("--version")]));
        assert!(requests_version(&[
            OsString::from("--database"),
            OsString::from("memory"),
            OsString::from("-V"),
        ]));
    }

    #[test]
    fn preserves_help_and_double_dash_semantics() {
        assert!(!requests_version(&[
            OsString::from("--help"),
            OsString::from("--version"),
        ]));
        assert!(!requests_version(&[
            OsString::from("remember"),
            OsString::from("--"),
            OsString::from("--version"),
        ]));
    }
}
