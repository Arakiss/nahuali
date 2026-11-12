---
name: nahuali
description: The working memory you share with the person and with other agents. Use it when you start work on a project (read where things stand), when a piece of work changes state, when you need the person to decide something, when something went wrong that should not happen twice, and before answering anything about past work or decisions. Not a log of what you did.
---

# Nahuali

One memory for the person and every agent that works for them. It answers
where things stand, what is waiting on the person, what was learned, and how
far each of those can be trusted. The person sees it as a board.

Eight verbs. Every one records who you are, the project, the commit and the
time for you.

## When you start

```sh
nahuali status
```

For this project: the questions waiting on the person, each piece of work
with its one-line card, and what was decided in the last seven days. Read
the decisions: an answer to a question you or another agent asked is there
and nowhere else. "Nothing held yet" is a correct answer.

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

`nahuali answer ASK "text"` closes an ask and makes it a decision. Answers
are the person's to give. Answer one yourself only to record what the
person told you in conversation, and say so in the text. If the question no
longer matters: `nahuali answer ASK "why" --withdraw`.

## When something went wrong that should not happen twice

```sh
nahuali learn "Agent worktrees start from the default branch, not from the branch you are on" \
  --moment "before spawning an agent in its own worktree" \
  --when "worktree add"
```

- The rule is what to do or to know. The moment is when it matters.
- `--when` is what that moment looks like in a command: a few words that
  must all be there. Give it whenever you can; without it the words of the
  moment are used, which is less exact. Repeat `--when` for other shapes of
  the same moment.
- `--everywhere` when it is not about this project. `--stop` when repeating
  the mistake is costly enough that the command should be refused once.

A lesson is not shown at the start of a session. It is shown when someone
is about to do the thing it is about.

## Before doing something that may have gone wrong before

```sh
nahuali check "git worktree add ../side -b side"
```

It prints the lessons whose moment this is, or "no lesson applies", which
is the usual answer. Where the tool you run in supports it, this happens by
itself before each command, and you see a lesson only when one applies. If
a command is refused with a lesson, read it; if you still mean the command,
run it again. If a lesson is no longer true: `nahuali retire NAME "why"`.

## Something worth knowing that is not the state of a work

```sh
nahuali note "The audit is on the 20th; the client sends the files a week before" \
  --tag dates --about importer
```

One or two sentences, with the words someone would look for. Not a diary.
`nahuali forget NAME "why"` when it goes stale.

## Before answering about the past

```sh
nahuali recall "audit date"
```

It returns the cards, decisions, lessons and notes that hold every word you
ask for, each with who said it, how sure they were and when. Ask with a few
exact words. "Nothing known" is a correct answer: say so instead of
guessing. `--all` looks in every project.

Under "CLOSE IN MEANING" it may add things that do not hold your words but
are about something alike. They are likenesses, not matches: read each one
and use it only if it is really about what you asked.

## Reach

When several people share the memory, `nahuali ask … --to NAME` puts the
question to that person instead of the one you work for, and `--mine` on
anything keeps it to the person you work for and their agents: use it for
what is personal, never for what the team needs to know.

Add `--private` to anything that must not be shown to work on another
project. `--all` on `status` and `recall` shows every project, without what
is private to another one.

## What it is not for

A diary of your session, a place to store long text, or a substitute for
commit messages. One card per piece of work, one line per card.
