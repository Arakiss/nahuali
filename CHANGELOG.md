# Changelog

All notable changes to this project are written here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-27

The first release: a working memory for a person and their agents, with
prebuilt binaries for macOS on Apple silicon and for Linux on x86_64 and
arm64.

### Added

- **Work, asks, decisions:** `status`, `update`, `ask` and `answer`. One card
  per piece of work, overwritten and with its past kept; questions for the
  person that stay in sight until answered, with what they block and the
  answer the agent recommends; the answer kept as a decision.
- **Lessons:** `learn`, `check`, `lessons` and `retire`. A rule learned from a
  failure is told to the next agent at the moment it is about to repeat it,
  and can stop the command once.
- **Notes and recall:** `note`, `forget` and `recall`, strict enough that an
  empty answer is a normal one. With `setup --meaning`, a local model
  (about 220 MB, on the CPU, nothing leaves the machine) also finds what was
  said in other words, kept apart from what the words find.
- **Four marks on everything:** who said it (an agent with its session), whether
  it was verified and by what, when and at which commit, and how far it reaches.
- **The board:** `nahuali` at a terminal, read and answered with the keyboard.
- **Agent tools:** `setup --hooks` brings the board to the start of a session
  and the lessons before a command in Claude Code, Codex, Antigravity and
  Cursor ([#8](https://github.com/Arakiss/nahuali/pull/8)); `mcp` serves the
  same verbs over the Model Context Protocol to any other.
- **Your machines and a team:** `node` brings the memories of your own machines
  together; `team` keeps a shared memory on a SurrealDB server, each person
  seeing what reaches them.
- **Its own health:** `doctor` checks every part and says what to do; `usage`
  reports how nahuali was used and where it got in the way
  ([#4](https://github.com/Arakiss/nahuali/pull/4),
  [#5](https://github.com/Arakiss/nahuali/pull/5)).
- **Releases:** a version is cut, built and published by itself when a change
  that a user meets lands on `main`.

[Unreleased]: https://github.com/Arakiss/nahuali/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Arakiss/nahuali/releases/tag/v0.1.0
