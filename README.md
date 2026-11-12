<p align="center">
  <img src="assets/readme/banner.png" alt="nahuali, a working memory for a person and their agents" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-being%20built-F7913D?style=flat-square&labelColor=1B100C" alt="Status: being built">
  <img src="https://img.shields.io/badge/runs-on%20your%20machine-F6F0E8?style=flat-square&labelColor=1B100C" alt="Runs on your machine">
  <img src="https://img.shields.io/badge/built%20on-SurrealDB%204-F6F0E8?style=flat-square&labelColor=1B100C" alt="Built on SurrealDB 4">
  <img src="https://img.shields.io/badge/works%20with-Claude%20Code%20%C2%B7%20Codex%20%C2%B7%20Cursor-F6F0E8?style=flat-square&labelColor=1B100C" alt="Works with Claude Code, Codex and Cursor">
</p>

# nahuali

**A working memory for a person and their agents. It says where things stand, what is waiting on you, and how far each of those can be trusted.**

When several agents work for you on several projects, the state of the work lives in chat scrollback and in each agent's head. Nahuali gives it one place. An agent says where its work stands and asks what it needs you to decide. You open one board, answer with one key, and the answer is where the agent looks next.

Nothing in it is a bare statement. Every card, question and decision carries who said it, whether it was verified or only claimed, when, and at which commit.

> [!NOTE]
> Nahuali is being built and has no release yet. It runs from source. Of the eight verbs in its design, four exist: `status`, `update`, `ask` and `answer`. The screens on this page are drawn by the real interface on invented data, and they show the lessons too, which the command does not write yet.

## Why

These happened in one working week, and each is a failure of memory, not of the model.

- After hours of work, nobody can say where things stand without digging through git and old messages.
- A question for the person is asked at the end of a long message, goes unanswered, and is asked again ten times because it lives nowhere.
- Nobody can tell what was verified from what an agent merely reported.
- Writing things down costs a forced paragraph at every stop.

## What it holds

| Thing | What it is | Its life |
| --- | --- | --- |
| Work | A named piece of work with one card: what is true now, what is done, what is next | The card is overwritten. Its past stays readable behind it |
| Ask | A question for you, with what it blocks and the answer the agent would give | Open and in sight until someone closes it. It never expires |
| Decision | An answered ask | Kept, with who decided |

Every one of them carries the same four marks.

| Mark | Says |
| --- | --- |
| who | You said it, an agent saw it, or an agent passes on what another told it |
| how sure | Verified, and by what, or only claimed |
| as of | The moment and, for work in a repository, the commit |
| reach | The project it belongs to, and whether it is private to it |

## What it looks like

<table>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/tui/board-120x36.png" alt="The board: two questions waiting on the person, three pieces of work in motion with their one-line cards, what was learned this week, and the axolotl looking worried because a question has waited two days">
      <br><sub>The board. What waits on you comes first, then the work, and how much of it is verified.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/tui/board-question-120x36.png" alt="A question opened: what it blocks, the recommended answer, its four marks and its past">
      <br><sub>A question, with what it blocks, what the agent recommends, and its marks.</sub>
    </td>
  </tr>
  <tr>
    <td width="50%" valign="top">
      <img src="docs/screenshots/tui/board-work-120x36.png" alt="A piece of work opened: its card, what is done, what is next, its four marks and every earlier state of the card">
      <br><sub>A piece of work. The card says what is true now, and its past is one step behind.</sub>
    </td>
    <td width="50%" valign="top">
      <img src="docs/screenshots/tui/board-80x24.png" alt="The same board in a terminal of 80 by 24 characters">
      <br><sub>The same board at 80×24. The axolotl falls back to half blocks where a terminal has no images.</sub>
    </td>
  </tr>
</table>

The axolotl is not decoration. It is worried when a question has waited two days, listening when one is open, thinking when work is blocked, and happy when nothing waits on you.

## How agents use it

Through one small command. The project, the commit and the time are recorded for the agent, so it adds only judgement.

```sh
nahuali status                       # before starting: open asks, the cards, what was decided lately
nahuali update importer \
  --line "tests green, review left" \
  --done "parser for the old format" \
  --next "open the review" \
  --verified "cargo test, 42 passing"   # without this the update is recorded as only claimed
nahuali ask "Ship behind a flag, or wait for the audit?" \
  --work importer --blocks "release 0.4" --recommends "behind a flag"
nahuali answer k3f9q2 "Behind a flag."  # closes the ask. It is now a decision
```

This is what an agent reads when it starts.

```text
atlas · 1 waiting on the person · 2 pieces of work

WAITING ON THE PERSON
  ask:k3f9q2  Ship behind a flag, or wait for the audit?
              blocks release 0.4 · holds importer · recommends behind a flag
              asked by claude-code, an agent · 2 d ago

WORK
  importer  waiting on ask:k3f9q2 · tests green, review left
            done: parser for the old format
            next: open the review
            claude-code · verified by cargo test, 42 passing · 12 min ago · commit abc1234
  pricing   not started
            codex · claimed · 1 d ago

DECIDED IN THE LAST 7 DAYS
  ask:9x8y7z  Which of the two logos goes on the cover?
              answer: The blue one.
              decided by ada · 3 h ago
```

A piece of work is never set to "waiting on the person" by hand. It waits while an open ask names it and stops waiting when that ask is closed. An agent that answers an ask itself is shown as an agent.

What an agent is told about Nahuali is one page: [`skills/nahuali/SKILL.md`](skills/nahuali/SKILL.md).

## The memory is yours

The memory lives in a database on your own machine, an embedded [SurrealDB](https://surrealdb.com), and comes out of it as plain lines of text whenever you ask.

```text
~/.local/share/nahuali/
  db/                  the database
  log/*.jsonl          lines to take in, from an export or from another machine
```

| Property | In practice |
| --- | --- |
| Nothing to install or manage | The first command that needs the memory starts the one process that keeps the database open. It leaves by itself when nobody speaks to it. No service, no password |
| Many at once | The board stays open while a dozen agents write. They all talk to that one keeper |
| Out as plain text | `nahuali export` prints every event as one line of JSON, with the time, who wrote it, how sure they were and what happened |
| In again, anywhere | A memory takes in the lines it finds in its `log/` folder, each one once. A backup, a move to another machine and two machines sharing a memory are all that same step |
| Order does not matter | The state is the events in order of their time. The same events give the same state wherever they came from and however often they arrive |

The whole format is in [`docs/format.md`](docs/format.md).

## How it works

```text
agents ── nahuali update / ask ──▶ the keeper ◀── nahuali answer ── you, on the board
                                       │
                                       ├─ SurrealDB: every event, added and never changed
                                       └─ folded in order of time ─▶ cards, open asks, decisions
```

Everything that happens is one event, added and never changed. What is true now is those events folded in order, so any card can be explained by the events behind it.

The rules live in [`nahuali-core`](crates/nahuali-core), which has no clock, no network and no process of its own, and does not know where events are kept. Whatever runs it says what time it is and who is speaking. Behind the command it is kept by SurrealDB. It also ships a store of plain files with no database at all, for a machine too small or too simple to run one.

| Crate | What it is |
| --- | --- |
| [`nahuali-core`](crates/nahuali-core) | The events, the fold and the verbs |
| [`nahuali-cli`](crates/nahuali-cli) | The `nahuali` command, and the keeper of the database |
| [`nahuali-view`](crates/nahuali-view) | The screens as a view model, with no dependencies |
| [`nahuali-tui`](crates/nahuali-tui) | The terminal renderer, with the animated axolotl |

## Running it

It needs a Rust toolchain.

```sh
cargo install --path crates/nahuali-cli
nahuali help
```

To see the interface on invented data, without a memory of your own:

```sh
cargo run -p nahuali-tui --example board
```

## Self-hosted, and a cloud beside it

Nahuali is meant to be open source that you run on your own machine or server, with a hosted service beside it for those who would rather not run it. The hosted service will run this same program, one separate installation per customer. Nothing is held back from the open version to make the hosted one better.

This repository is kept, from its first commit, as if it were already public. What belongs in it and what never does is in [`docs/boundaries.md`](docs/boundaries.md).

## What it does not do yet

- It does not prove who is speaking. "Who" is what the speaker declared.
- It does not encrypt. The memory is as private as the disk it is on.
- It does not run on Windows yet.
- It has no lessons, notes or recall yet. They are next.

## Roadmap

In this order, each when it is done. There are no dates.

1. ~~The cards and the asks, the verbs `status`, `update`, `ask` and `answer`, and the board on real work.~~
2. Lessons, and `check`. An agent says what it is about to do and is told the lessons that apply, at that moment.
3. Notes and `recall`, strict enough that an empty answer is a normal one.
4. The same verbs over MCP, so no agent tool is special.
5. Releases you can install without a toolchain.

The definition the design starts from is [`docs/product.md`](docs/product.md).

## Brand

In Mesoamerican tradition a nahual is a person's animal double. The axolotl is ours. It never finishes growing up and it grows back what it loses, which is a fair description of a memory worth having. The name Nahuali and the axolotl are not covered by the licence of the code ([`assets/README.md`](assets/README.md)).

## License

Not chosen yet. It is chosen on the day this repository is published.
