# How the memory is kept

A Nahuali memory is a folder of plain text. You own it, you can read it
without Nahuali, and you can copy it with any tool that copies files.

```
<home>/README.txt            what these files are, for whoever finds them
<home>/log/<writer>.jsonl    everything that happened, one line per event
<home>/local/writer          this installation's name; stays on this machine
```

`<home>` is `--home`, then `NAHUALI_HOME`, then `$XDG_DATA_HOME/nahuali`,
then `~/.local/share/nahuali`. Reading never creates it.

## One line, one event

```json
{"v":1,"id":"01M786GSCH9BNF470HFAZ58K88","at":"2026-11-12T15:37:45Z","by":{"agent":"claude-code"},"project":"atlas","sure":{"verified":"cargo test"},"commit":"abc1234","what":{"work":{"work":"importer","line":"tests green, review left"}}}
```

| Field | Meaning |
|---|---|
| `v` | The version of the format. This is version 1. |
| `id` | Unique, and sorts by the moment it was made (a ULID). |
| `at` | When, in UTC. |
| `by` | Who: `{"person":"ada"}`, `{"agent":"claude-code"}`, or `{"reported":{"by":"lead","from":"scout"}}` when one passes on what another said. |
| `project` | Where it belongs. |
| `private` | Present and `true` when it must not be offered to work on another project. |
| `sure` | `"claimed"`, or `{"verified":"by what"}`. |
| `commit` | For work in a repository: the commit it was true at. |
| `what` | What happened: one of the kinds below. |

The first five and `sure` are the four marks of `docs/product.md`: who, how
sure, as of, and reach. No line exists without them.

| Kind | Fields | Effect |
|---|---|---|
| `work` | `work`, and any of `title`, `goal`, `status`, `line`, `done`, `next` | Overwrites that work's card. A field left out keeps what it had; `done` adds; `next` replaces. The first one starts the work. |
| `ask` | `ask`, `question`, and optionally `blocks`, `recommends`, `work` | Opens a question for the person. |
| `answer` | `ask`, `text` | Closes the ask. It is now a decision. |
| `withdraw` | `ask`, `why` | Closes the ask without an answer. |

`status` is `"moving"`, `"not-started"`, `"done"` or `{"blocked":"by what"}`.
"Waiting on the person" is never written: a work waits while an open ask
names it, and stops waiting when that ask is closed.

## What is true now

The state is every line of every file in `log/`, put in order of `at` and
then `id`, and applied one after another. Nothing else is stored: there is
no index and no database to fall out of step with the files.

Three rules make that order enough:

- A card is whatever the latest `work` line left it as.
- The first close of an ask stands. A later one changes nothing.
- A work marked private stays private, whatever later lines say.

Because the order comes from the lines and not from the files, the same
lines give the same state however they are split, copied or repeated.

## Several writers

Each installation appends only to its own file, named by `local/writer`.
Several agents on one machine share that file and take turns.

Two machines share a memory by copying `log/` both ways, with rsync, a
synced folder, git, or a USB stick. They never write the same file, so
there is nothing to merge and nothing to conflict. Do not copy `local/`.

If both machines closed the same ask before they met, both end up agreeing
on the earlier close. If both updated the same card, the later update is
the card and the earlier one is in its past.

## What a reader tolerates

A reader never refuses a log because of one line.

- A line it cannot parse (torn by a crash, or edited by hand) is skipped and
  counted. `status` says how many.
- A line whose `v` is higher than it knows is skipped and counted.
- A kind of `what` it does not know is skipped and counted.
- A field it does not know is ignored.

So two installations on different versions can share a log, and the older
one loses only what it could not have understood.

A crash in the middle of a write leaves half a line at the end of a file.
The next write closes it first, so the half line spoils itself and nothing
after it.

## What it does not do

- It does not prove who is speaking. `by` is what the speaker declared: a
  tool that runs agents names them, and an agent can name itself. An agent
  that answers an ask is shown as an agent.
- It does not encrypt. The folder is as private as the disk it is on.
- It does not compact. Every line is read on every command. That is fast
  for the tens of thousands of lines a person and their agents write in
  years; past that it will need a snapshot, which can be rebuilt from the
  lines at any time.
