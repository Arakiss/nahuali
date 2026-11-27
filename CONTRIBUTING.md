# Contributing

Issues and pull requests are welcome; for anything large, open an issue first
so we can agree on the shape.

## Before a pull request

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets
cargo test --workspace
ops/privacy-check.sh
```

The tests of a team need [`surreal`](https://surrealdb.com/install) 4 on your
`PATH`. Without it they are skipped locally and fail in CI.

## Commits and releases

Commit messages are [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/):
`fix(team): wait for the keeper to leave`, then a blank line and a body that
says why. `feat` and `fix` are what a user meets and make a release; a
change that breaks something carries `!` and a `BREAKING CHANGE:` footer.
Install the hook that checks each message, and that it holds nothing
private: `ln -s ../../ops/commit-msg.sh .git/hooks/commit-msg`.

Versions follow [Semantic Versioning](https://semver.org), and
`CHANGELOG.md` follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Both are kept by `ops/release`, and a release cuts itself: when CI passes on
`main` and a change a user meets has landed since the last tag, the Release
workflow runs `ops/release cut` (the next version, every crate bumped, the
changelog written, an annotated tag), pushes it, builds the binaries on each
platform and publishes them with the notes. `ops/release plan` says locally
what the next release would be. The notes are drawn from the commits, each
linked to its pull request; to say more than the commits do, write it under
`## [Unreleased]` in `CHANGELOG.md` and those words are the notes instead.
CI runs `ops/release check` so that tags, crates and changelog never disagree.

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
