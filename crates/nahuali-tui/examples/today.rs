//! Interactive demo on the invented mock mind.
//!
//! ```text
//! cargo run -p nahuali-tui --example today
//! cargo run -p nahuali-tui --example today -- --halfblocks --motion reduced --256
//! ```
//!
//! Keys: 1-5 or tab to switch views, j/k to move, enter to open, c to close
//! an intention, / to search, ? for every key, m to cycle the axolotl's
//! states, ! to break and repair the integrity chain, q to quit.

use std::io;

use nahuali_tui::anim::Motion;
use nahuali_tui::theme::ColorMode;
use nahuali_tui::{Options, run};
use nahuali_view::mock::MockSource;

fn main() -> io::Result<()> {
    let mut options = Options { demo: true, ..Options::default() };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--halfblocks" => options.force_halfblocks = true,
            "--256" => options.color = ColorMode::Ansi256,
            "--truecolor" => options.color = ColorMode::TrueColor,
            "--motion" => {
                let value = args.next().unwrap_or_default();
                options.motion = Motion::parse(&value).unwrap_or_else(|| {
                    eprintln!("--motion takes full, reduced or none");
                    std::process::exit(2);
                });
            }
            "--fps" => {
                options.fps = args.next().and_then(|v| v.parse().ok()).unwrap_or(30);
            }
            other => {
                eprintln!("unknown flag {other}; flags: --halfblocks --256 --truecolor --motion M --fps N");
                std::process::exit(2);
            }
        }
    }
    let src = MockSource::new();
    let toggle = || src.set_chain_broken(!src.chain_broken());
    run(&src, options, Some(&toggle))
}
