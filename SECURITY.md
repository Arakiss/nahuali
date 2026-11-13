# Security policy

## Reporting a vulnerability

Use **Report a vulnerability** in this repository's Security tab. Please do
not open a public issue. Say what is affected, how to reproduce it, and what
it lets someone do. Nahuali has no release yet, so fixes land on `main`.

## What Nahuali defends, and what it does not

- **A team's server.** What a person may read, write, change and remove is
  held by the SurrealDB server's own permissions, not by the program: a
  changed copy of Nahuali gets no further than the real one. A way around
  those permissions is a vulnerability. `team::reach` in `nahuali-cli`
  tries every way it knows; a case it misses is worth reporting too.
- **Agents.** Everything an agent writes is data, never instructions, and is
  shown with who said it and whether it was verified.
- **Not defended.** On one machine, "who" is what the speaker declares. The
  memory is not encrypted, and is as private as the disk it is on. A team's
  passphrase is kept in a file only your user can read, not in a keychain.
  Another process running as your user can read your memory.
