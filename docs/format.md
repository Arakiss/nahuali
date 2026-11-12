# How the memory is kept

A Nahuali memory lives in a database on your own machine, and comes out of
it as plain lines of text whenever you ask.

```
<home>/db/                   the database: an embedded SurrealDB
<home>/log/*.jsonl           lines to take in: from an export, or from another machine
<home>/local/keeper.log      what the keeper said; stays on this machine
```

`<home>` is `--home`, then `NAHUALI_HOME`, then `$XDG_DATA_HOME/nahuali`,
then `~/.local/share/nahuali`. Reading a memory that does not exist creates
nothing.

## The keeper

A database on disk is opened by one process at a time, and a memory is used
by several at once: the board stays open while agents write. So one process
has the database open, the keeper, and every command talks to it.

- The first command that needs the memory starts a keeper. Nothing is
  installed, nothing asks for a password, and there is no service to manage.
- Commands reach it over a socket that only your user can open.
- It leaves by itself after ten minutes of silence, and the next command
  starts another. `nahuali stop` makes it leave now.
- Opening the database is what makes it the only one: a second keeper for
  the same memory is refused.
- A keeper started by an older build steps aside when a newer command
  speaks to it.

The database is SurrealDB, embedded in the `nahuali` program itself. The
same code can speak to a SurrealDB server instead, which is how a team or a
hosted installation would run it.

## One event, one record, one line

Everything that happens is one event. In the database it is a record of the
table `event`. Out of it, it is one line of JSON:

```json
{"v":1,"id":"01M786GSCH9BNF470HFAZ58K88","at":"2026-11-12T15:37:45Z","by":{"agent":"claude-code"},"project":"atlas","sure":{"verified":"cargo test"},"commit":"abc1234","what":{"work":{"work":"importer","line":"tests green, review left"}}}
```

| Field | Meaning |
|---|---|
| `v` | The version of the format. This is version 1. |
| `id` | Unique, and sorts by the moment it was made (a ULID). It is the record's id. |
| `at` | When, in UTC. |
| `by` | Who: `{"person":"ada"}`, `{"agent":"claude-code"}`, or `{"reported":{"by":"lead","from":"scout"}}` when one passes on what another said. |
| `project` | Where it belongs. |
| `private` | Present and `true` when it must not be offered to work on another project. |
| `sure` | `"claimed"`, or `{"verified":"by what"}`. |
| `commit` | For work in a repository: the commit it was true at. |
| `what` | What happened: one of the kinds below. |

The first five and `sure` are the four marks of `docs/product.md`: who, how
sure, as of, and reach. No event exists without them.

| Kind | Fields | Effect |
|---|---|---|
| `work` | `work`, and any of `title`, `goal`, `status`, `line`, `done`, `next` | Overwrites that work's card. A field left out keeps what it had; `done` adds; `next` replaces. The first one starts the work. |
| `ask` | `ask`, `question`, and optionally `blocks`, `recommends`, `work` | Opens a question for the person. |
| `answer` | `ask`, `text` | Closes the ask. It is now a decision. |
| `withdraw` | `ask`, `why` | Closes the ask without an answer. |
| `learn` | `lesson`, `rule`, `moment`, and optionally `when`, `everywhere`, `stop` | Records a lesson and the moment it applies. |
| `retire` | `lesson`, `why` | The lesson stopped being true. |
| `note` | `note`, `text`, and optionally `tags`, `about` | Keeps something worth knowing. |
| `forget` | `note`, `why` | The note went stale or was wrong. |
| `shown` | `lessons`, `doing`, and optionally `session`, `stopped` | Lessons were shown to someone about to act. Written by the machine. |

`status` is `"moving"`, `"not-started"`, `"done"` or `{"blocked":"by what"}`.
"Waiting on the person" is never written: a work waits while an open ask
names it, and stops waiting when that ask is closed.

A record keeps the line whole, and beside it the fields to query by: `at`
as a time, `project`, `by`, whether a person said it, whether it was
verified, whether it is private, and the kind.

## What is true now

Events are only ever added. The state is every event, in order of `at` and
then `id`, applied one after another.

- A card is whatever the latest `work` event left it as.
- The first close of an ask stands. A later one changes nothing.
- A work marked private stays private, whatever later events say.
- A lesson is in force until it is retired, and a note is kept until it is
  forgotten. Neither is ever changed: a better one is a new one.

Because the order comes from the events themselves, the same events give
the same state wherever they came from and however often they arrive.

## When a lesson applies

No model is asked. What is about to be done and what marks a lesson's
moment both become sets of plain terms: lower case, accents folded, split
where letters end, common words dropped, cut to a rough root.

- With `when` given, one entry is enough, and every term of that entry has
  to be in what is about to be done, within eight terms of each other. A
  script of two hundred lines has every word somewhere; a command has them
  together.
- Without it, the moment's own words are used: three in five of them have
  to be there, near each other, and never fewer than two unless the moment
  has only one.

A lesson reaches the project it was learned in, or every project when it
says so and is not private. A lesson marked `stop` refuses the command once
per session; the second time it goes through. Every showing is an event,
so a lesson knows how often it was shown and how often it stopped an act.

## What can be searched

The table `thing` holds one record for each card, open ask, decision,
lesson in force and note, as they stand now, with a full-text index over
their words (lower case, accents folded, English roots, ranked by BM25).
It is derived from the events and rebuilt when a keeper starts, so it can
never disagree with them for longer than one write.

`recall` returns the things that hold every word asked for, as far as the
asker's reach goes: its own project and what is meant for every project,
or with `--all` everything that is not private to another project. What
was withdrawn, retired or forgotten is not there to be found.

## Meaning

`nahuali setup --meaning` installs a multilingual model of 107 million
parameters under `<home>/models` (220 MB; each file is checked against a
pinned hash). The keeper loads it when it starts and runs it on the CPU.
Nothing is sent anywhere.

With it, each record of `thing` also carries `vec`, a vector of 384
numbers for its words, and the table `vector` remembers every vector
already computed by the text it is of, so a keeper that starts again
computes only what is new. Vectors are derived: they are in no event and
in no export, and deleting them loses nothing.

A question is turned into a vector the same way and compared with every
thing the asker may see, in the database, by cosine likeness. What is at
least 0.65 alike is offered, after what the words found, apart from it,
and named a likeness. `NAHUALI_NEAR` moves the line. The measurement
behind it is `cargo run --release -p nahuali-embed --example near`.

For lessons the same holds, with two limits. A command longer than 400
characters is not compared by meaning. And a lesson that only meaning
brought is told and never stops a command: only its words do that.

At the size of one person's memory every vector is compared. SurrealDB's
approximate index (HNSW) answers the same question for millions; it is not
defined yet because it is not needed yet.

## Out, and in again

`nahuali export` prints every event as one line. That file is yours: it
can be read without Nahuali, kept as a backup, or put under version control.

A memory takes in every `*.jsonl` file under its `log/` folder when its
keeper starts. An event that is already there is not taken twice, and a
line that cannot be read is left out and counted. So:

- **A backup** is the output of `nahuali export`.
- **Moving to another machine** is putting that file in the other memory's
  `log/`.
- **Two machines sharing a memory** is each exporting to a file the other
  takes in, with any tool that copies files. Nothing has to be merged: if
  both closed the same ask before they met, both end up agreeing on the
  earlier close, and if both updated the same card, the later update is the
  card and the earlier one is in its past.

## What a reader tolerates

- A line it cannot parse (torn, or edited by hand) is left out and counted.
- A line whose `v` is higher than it knows is left out.
- A kind of `what` it does not know is kept and has no effect.
- A field it does not know is ignored.

So two installations on different versions can exchange lines, and the
older one loses only what it could not have understood.

## Without a database

The part that holds the rules, `nahuali-core`, does not know where events
are kept. It also ships a journal that keeps them as plain files, with no
database and no keeper, for tests and for a machine too small or too simple
to run one. The format of the lines is the same.

## What it does not do

- It does not prove who is speaking. `by` is what the speaker declared: a
  tool that runs agents names them, and an agent can name itself. An agent
  that answers an ask is shown as an agent.
- It does not encrypt. The memory is as private as the disk it is on.
- It reads every event on every command and folds them in the program.
  That is fast for what a person and their agents write in years.
- Meaning is a likeness. It finds about half of what is said in other
  words, and it is kept apart from what the words find.
- The keeper speaks over a Unix socket. Windows is not supported yet.

## SurrealDB's licence

SurrealDB is under the Business Source License 1.1 until 2030-01-01, when
it becomes Apache 2.0. Its grant allows any use except offering it as a
"Database Service", which the licence defines as an offering that lets
third parties create, manage or control schemas or tables. Running Nahuali
for yourself is plainly allowed. A hosted Nahuali does not hand schemas to
its customers, so it reads as allowed too; that reading should be confirmed
before a hosted service depends on it.
