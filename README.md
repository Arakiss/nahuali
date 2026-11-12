# Nahuali

A working memory for a person and their agents: where things stand, what is
owed to whom, what was learned, and how far each of those can be trusted.

This repository starts again from an empty design. The previous
implementation is archived and nothing here depends on it. What was kept is
what a person sees:

- `crates/nahuali-view`: the screens as a view model, with no dependencies.
- `crates/nahuali-tui`: the terminal renderer for that model.
- `assets/`: the mascot and the rest of the art.

To see the terminal interface on sample data:

```sh
cargo run -p nahuali-tui --example today
```
