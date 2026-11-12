---
name: nahuali
description: The working memory you share with the person and with other agents. Use it when you start work on a project (read where things stand), when a piece of work changes state, and when you need the person to decide something. Not a log of what you did.
---

# Nahuali

One memory for the person and every agent that works for them. It answers
where things stand, what is waiting on the person, and how far each of
those can be trusted. The person sees it as a board.

## When you start

```sh
nahuali status
```

It prints, for this project: the questions waiting on the person, each
piece of work with its one-line card, and what was decided in the last
seven days. Read the decisions: an answer to a question you or another
agent asked is there and nowhere else. If it prints "nothing held yet",
that is a correct answer.

## When the state of your work changes

```sh
nahuali update importer --line "tests green, review left" \
  --done "parser for the old format" --next "open the review" \
  --verified "cargo test, 42 passing"
```

- The name (`importer`) is yours to choose: lower-case, digits, dashes.
  The first update starts the work; later ones overwrite its card.
- `--line` is what is true now, in one line. Not what you did.
- `--verified "by what"` only when you checked it yourself, just now, and
  can say how. Without it the update is recorded as claimed. Never write
  `--verified` for something another agent told you; use `--from NAME`.
- `--blocked "by what"` when it cannot go on; `--status done` when it is
  finished.
- The time, the project and the commit are recorded for you.

Update when something changed. Not at every stop, and not to narrate.

## When the person has to decide

```sh
nahuali ask "Ship the importer behind a flag, or wait for the audit?" \
  --work importer --blocks "release 0.4" --recommends "behind a flag"
```

Ask here instead of at the end of a message. The question stays on the
person's board until it is answered, and the work it names shows as
waiting on them. Always give `--recommends`: the answer you would give.
Do not repeat the question in chat. Go on with whatever does not depend
on it, and find the answer in `nahuali status` later.

If the question no longer matters:

```sh
nahuali answer k3f9q2 "the importer was dropped" --withdraw
```

## Answering

`nahuali answer ASK "text"` closes an ask and makes it a decision. Answers
are the person's to give. Answer one yourself only when the person told
you the answer in conversation, and say so in the text.

## Reach

Add `--private` to anything that must not be shown to work on another
project. `nahuali status --all` shows every project, without what is
private to another one.

## What it is not for

A diary of your session, a place to store long text, or a substitute for
commit messages. One card per piece of work, one line per card.
