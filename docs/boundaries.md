# What belongs in this repository

Nahuali is meant to be open source that anyone can host themselves, with a
hosted service beside it for those who would rather not. This repository is
the first of the two, so it is kept, from its first commit, as if it were
already public. Making it public should need a licence file and nothing else.

## In here

Everything a person needs to run Nahuali on their own machine or server and
to change it:

- the program: what holds the memory, the command line, the protocol agents
  speak, and the terminal interface;
- its tests, on invented data only;
- its documentation;
- the art the program draws at run time: the axolotl's sprite sheet, its
  animation frames and their manifest.

## Never in here

| What | Why | Where it lives |
|---|---|---|
| Anyone's real memory, or anything copied from one: names, clients, money, health, addresses | A fixture is forever in history | Nowhere in a repository; tests use the invented cast (`ada`, `atlas`, `borealis`, `cobalt`) |
| Home paths, machine names, personal e-mail addresses, in files or in commits | They identify a person | Commits carry a public identity; examples use `example.com` |
| Secrets of any kind | Obvious, and irreversible | A secret store, never a file here |
| The hosted service: accounts and billing, the fleet that runs many installations, its infrastructure and its operations | It is the business, and it is not needed to self-host | A private repository of its own |
| Design sources of the art: source frames, sketches, how each image was made | The name and the axolotl are the brand, not part of the code's licence | A private repository of its own |
| One person's installation: their scripts, their machine's setup, their working notes | It is not the product | That person's own files |

A rule of thumb for the second row from the bottom up: if a stranger
self-hosting Nahuali would never need it, it does not belong here.

## The line between the two offerings

The hosted service runs this same program for each customer, one separate
installation per customer, as a hosted database service runs one database
per project. Nothing is held back from the open version to make the hosted
one better: the hosted one sells not having to run it. What stays private is
only what exists because there are many customers.

So a feature belongs here when a single installation needs it, and in the
hosted service's repository when it only exists because there are many.

## Licence

Not chosen yet, on purpose: it is chosen on the day this repository is
published. Until then the crates say `LicenseRef-Private`. The working
assumption is a permissive licence for the code. Whatever it is, it will not
cover the name Nahuali or the axolotl (see `assets/README.md`).

## How the line is kept

- `ops/privacy-check.sh` fails on home paths, on e-mail addresses outside
  documentation domains, and on any term of a private deny-list that is read
  from a file outside the repository (`NAHUALI_PRIVACY_DENYLIST`), so the
  list of real names never has to be committed. `--self-test` plants one
  violation per rule and expects each to be caught.
- `ops/pre-commit.sh` runs that check and refuses a commit signed with a
  personal e-mail address. Install it with
  `ln -s ../../ops/pre-commit.sh .git/hooks/pre-commit`.
- An exception is one exact line in `ops/privacy-allow.txt`, pinned by its
  hash, with the reason beside it.
