# Nahuali

A working memory for a person and their agents: where things stand, what is
owed to whom, what was learned, and how far each of those can be trusted.

An agent says where its work stands and asks what it needs to know. The
person sees one board, answers with one key, and the answer is where the
agent looks next. Everything carries who said it, how sure they were and as
of when.

```sh
cargo install --path crates/nahuali-cli

nahuali update importer --line "tests green, review left" --verified "cargo test"
nahuali ask "Ship behind a flag, or wait for the audit?" --work importer --recommends "behind a flag"
nahuali status      # what an agent reads before it starts
nahuali             # the board, at a terminal
```

`nahuali help` lists the rest. The memory is a folder of plain text files
that you own and can read without Nahuali: `docs/format.md`.

## What is here

- `crates/nahuali-core`: what is held and how it is kept. A log of plain
  lines, folded into state. No clock, no network, no process of its own.
- `crates/nahuali-cli`: the `nahuali` command.
- `crates/nahuali-view`: the screens as a view model, with no dependencies.
- `crates/nahuali-tui`: the terminal renderer for that model.
- `skills/nahuali`: what an agent is told about it.
- `assets/`: the axolotl as the program draws it.

To see the terminal interface on sample data, without a memory of your own:

```sh
cargo run -p nahuali-tui --example board
```

## Where this is going

`docs/product.md` is the definition the design starts from. Of its eight
verbs, four exist: `status`, `update`, `ask` and `answer`. Lessons and
`check` come next, then notes and `recall`.

What belongs in this repository and what never does is in
`docs/boundaries.md`.
