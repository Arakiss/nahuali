# Nahuali: the product

Nahuali is a working memory for a person and their agents. It answers four
questions at any moment, for the person and for whichever agent is at work:
where things stand, what is owed to whom, what was learned, and how far each
of those can be trusted.

This page is the definition the design starts from. It was written before
any code or architecture, and it replaces nothing: there is no earlier model
to stay compatible with.

## Who it is for

First, one person who works with several agents, in several tools (Claude
Code, Codex, Cursor), on several projects at once. Then a small team. A
company comes last, and only after the first two use it every day.

## What goes wrong today

Each of these happened in one working week, and each is a failure of memory,
not of the model.

- After hours of work, neither the person nor the agent can say where things
  stand without digging through git and old messages.
- A question for the person is asked at the end of a long message, goes
  unanswered, and is repeated ten times because it lives nowhere.
- A lesson is written down somewhere and arrives after the mistake is made
  again.
- Automatic recall brings in something unrelated, once something private.
- Nobody can tell what was verified from what an agent merely reported.
- Writing things down costs a forced paragraph at every stop.

## What it holds

Five kinds of thing, and no more until one of them proves too small.

| Thing | What it is | Its life |
|---|---|---|
| Work | A named piece of work with one state card: goal, status, done, next, where | The card is overwritten; its past stays readable behind it |
| Ask | A question for someone, with what it blocks and a recommended answer | Open until answered or withdrawn, and visible the whole time |
| Decision | A choice and its reason; usually an answered ask | Kept; a later decision can replace it and says so |
| Lesson | A rule learned from a failure, with the moment it applies | Shown when that moment comes; retired when it stops being true |
| Note | Something worth knowing | Kept until it goes stale or is forgotten |

Every one of them carries the same four marks:

- **who**: said by the person, observed by an agent, or reported by another agent;
- **how sure**: verified (and by what), or only claimed;
- **as of**: a date and, for anything about code, the commit it was true at;
- **reach**: the project it belongs to, and whether it is private.

## What it promises

1. **State before history.** The first screen says where each piece of work
   stands in ten seconds. History is one step behind it, never in front.
2. **Nothing owed is lost.** Every open ask stays in view, with what it
   blocks, until someone answers or withdraws it. It is never restated in chat.
3. **The lesson arrives at the moment of the act.** An agent about to do the
   thing a lesson is about is told then, not at the start of the session.
4. **Strict recall.** Nothing is offered unless it clearly belongs to the
   work at hand. Nothing private is offered to work it does not belong to.
   An empty answer is a correct answer.
5. **Provenance in sight.** No item is shown without who said it, how sure
   it is and as of when.
6. **Writing costs almost nothing.** What a machine can know (commits, files
   touched, time) it records itself. An agent adds only judgement: why, and
   what is next. No stop demands a paragraph.
7. **Any tool.** One small protocol, usable from a command line and over
   MCP. No agent tool is special.
8. **Yours.** Local first, private by default, in a database on the person's
   own machine, and out of it at any time as plain lines they can read
   without Nahuali.

## What an agent does with it

Eight verbs, each one line.

| Verb | What it does |
|---|---|
| `status` | Read the cards and open asks of this project |
| `update` | Overwrite a work's card: status, done, next |
| `ask` | Put a question to the person, with what it blocks and a recommendation |
| `answer` | Close an ask; it becomes a decision |
| `learn` | Record a lesson and the moment it applies |
| `check` | Say what is about to be done and get the lessons that apply |
| `note` | Keep something worth knowing, with its marks |
| `recall` | Ask for what is known about something; may return nothing |

## What a person sees

The terminal interface, with the axolotl. Its first screen is the whole
product in one glance:

```
 NAHUALI                                                    ada · thursday
 ───────────────────────────────────────────────────────────────────────────
 WAITING ON YOU  2
  ▸ atlas     Ship the importer behind a flag, or wait for the audit?
              blocks: release 0.4 · asked 2 days ago · recommends: behind a flag
  ▸ borealis  Which of the two logos goes on the cover?
              blocks: nothing yet · asked today

 IN MOTION  3
  ● atlas     importer       tests green, review left          12 min ago
  ● borealis  cover          blocked: waiting on you            3 h ago
  ○ cobalt    pricing page   not started                        yesterday

 LEARNED THIS WEEK  2
  ! before spawning agents in worktrees: they start from the default branch
  ! nine build directories filled the disk: share one

 ───────────────────────────────────────────────────────────────────────────
 enter open · a answer · / search · ? keys                   all verified ✓
```

Opening any line shows its history and its four marks. Answering an ask is
one key.

## What it is not

- Not an archive of everything that was said.
- Not a memory service for other people's applications.
- Not a place where agents coordinate a fleet. That may come later.
- Not a compliance product.
- Not one big shared system: an installation serves one person or one team.

## How we will know it works

Measured on the person and the agents who use it first.

| Promise | Measure |
|---|---|
| State before history | "Where are we?" is answered from the first screen, with no git command, in under a minute after any break |
| Nothing owed is lost | No ask is ever repeated in chat |
| Lesson at the moment | A known mistake is stopped before it is repeated, and the stop is recorded |
| Strict recall | Of what recall offers, nine in ten are judged relevant, and nothing private and unrelated is ever offered |
| Writing costs almost nothing | At most one short update per stop, and none when nothing changed |

## The first thing to build

The smallest version the two of us would use tomorrow: the cards and the
asks, the verbs `status`, `update`, `ask` and `answer`, and the first
screen. This part exists: see `docs/format.md` for how it is kept.
Lessons and `check` come second. Notes and recall come third.

## Beyond a desk

Nothing here is built for a machine that acts in the world, and nothing
here stands in its way. A robot that works with people has the same four
questions: where things stand, what it is waiting to be told, what it
learned, and how far each of those can be trusted. Three choices are kept
so that the same memory could one day sit inside one:

- The part that holds the rules has no clock, no network and no process of
  its own. Whatever runs it says what time it is and who is speaking.
- It does not know where events are kept. Behind the command they are in a
  database; on a machine too small for one they can be plain files, which
  survive losing power in the middle of a line.
- "Who" is anything that can say or observe something. A person and a
  coding agent are the first two, not the only two.

## How it is offered

Open source, to run on your own machine or server, and a hosted service
beside it for those who would rather not run it. The hosted service runs
this same program, one separate installation per customer; nothing is held
back from the open version to make the hosted one better. What goes in this
repository and what stays out is in `docs/boundaries.md`.

## Decided later, on purpose

What a team adds, the licence, and what the hosted service costs. None of
them changes anything above. How it is stored and how two machines share it
were decided when the first part was built, and are in `docs/format.md`.
