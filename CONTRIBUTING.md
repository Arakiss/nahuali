# Contributing

Nahuali is being built and has no release yet. Issues and pull requests are
welcome; for anything large, open an issue first so we can agree on the shape.

## Before a pull request

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
ops/privacy-check.sh
```

The tests of a team need [`surreal`](https://surrealdb.com/install) 4 on your
`PATH`. Without it they are skipped locally and fail in CI.

## What a change carries

- Tests on invented data only. The cast is `ada`, `bob`, `atlas`, `borealis`
  and `cobalt`; never a real memory, name, client or machine.
- No home paths or personal e-mail addresses, in files or in commits. Install
  the hook that checks both: `ln -s ../../ops/pre-commit.sh .git/hooks/pre-commit`.
- A check that is added proves it can fail: plant the defect it exists for
  and show that it is caught.

What belongs in this repository and what never does is in
[`docs/boundaries.md`](docs/boundaries.md).

## Licence

Unless you say otherwise, a contribution you submit is licensed as the
project is, under either of MIT or Apache 2.0, with no additional terms.
