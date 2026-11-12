# Nahuali

A working memory for a person and their agents: where things stand, what is
owed to whom, what was learned, and how far each of those can be trusted.

This repository starts again from an empty design. The previous
implementation is archived and nothing here depends on it. What was kept is
what a person sees:

- `crates/nahuali-view`: the screens as a view model, with no dependencies.
- `crates/nahuali-tui`: the terminal renderer for that model.
- `assets/`: the axolotl as the program draws it.

To see the terminal interface on sample data:

```sh
cargo run -p nahuali-tui --example board
```

Where this is going is in `docs/product.md`. What belongs in this repository
and what never does is in `docs/boundaries.md`.
