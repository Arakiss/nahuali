# Your own machines

Every machine of yours keeps a whole memory and works with no network. When
they can see each other they bring their memories together by themselves.
One machine that stays on, at home, is enough for all of them to meet.

## The machine that stays on

On the computer that will answer the others (Linux, or Linux inside
Windows with WSL):

```sh
cargo install --path crates/nahuali-cli --locked
nahuali node listen
```

It prints one line to run on each other machine, with the key of this
memory in it. The key is drawn once and is the only thing the machines
share: whoever has it can read and write this memory, so pass it the way
you would pass a password.

A keeper leaves when nobody has used it for ten minutes. For this machine
to answer the others at any time, keep one running:

```sh
nahuali serve --stay
```

As a service of your user, with systemd:

```ini
# ~/.config/systemd/user/nahuali.service
[Unit]
Description=Nahuali, for my other machines

[Service]
ExecStart=%h/.cargo/bin/nahuali serve --stay
Restart=on-failure

[Install]
WantedBy=default.target
```

```sh
systemctl --user enable --now nahuali
loginctl enable-linger "$USER"     # so it runs before you log in
```

### Inside Windows, with WSL

WSL keeps Linux on a network of its own, so by default the other machines
on your Wi-Fi cannot reach a port opened inside it. Two ways out:

- Mirrored networking (Windows 11): in `%UserProfile%\.wslconfig`, under
  `[wsl2]`, set `networkingMode=mirrored`, then `wsl --shutdown`. Linux then
  shares the computer's own address.
- Or forward the port from Windows to WSL with `netsh interface portproxy`.

Either way, Windows' firewall has to let port 8746 in from the private
network. systemd has to be turned on in WSL (`/etc/wsl.conf`, `[boot]`,
`systemd=true`) for the service above.

This page was written from how WSL is documented to work. The meeting of
machines is tested on one computer over its own loopback and on macOS; it
has not yet been run on Linux or inside WSL.

## Every other machine

```sh
NAHUALI_KEY=… nahuali node join pc.local:8746
```

From then on, whenever `nahuali` is in use on this machine, it goes to the
other every half minute: it takes the events it lacks and gives the ones
the other lacks. `nahuali node sync` does it now, and `nahuali node` says
how the last time went. With the other machine off or out of reach,
nothing changes: this one works on, and catches up when they meet again.

## What happens when they meet

A memory is a set of events that are never changed. Two machines that meet
tell each other which events they hold, and each takes the ones it lacks.
There is nothing to merge: the state is the events in order of their time,
so the same events give the same state on every machine.

- If two machines updated the same card while apart, the later update is
  the card and the earlier one is in its past.
- If both closed the same ask, both agree on the earlier close.
- A lesson learned on one is told on all of them.
- Two laptops that only ever go to the machine at home end up holding the
  same: they meet through it.

The search index and the vectors are not exchanged. Each machine derives
its own from the events, with or without the model.

## What goes over the network

Each message between two machines is sealed with the memory's key
(XChaCha20-Poly1305). Someone else on the same Wi-Fi can see that two
machines are talking and how much, and nothing else: they cannot read a
message, change one, or put an event in. A machine with another key is
told nothing and is taken nothing from.

What this does not do:

- It does not hide the key from someone who can read your files. The key is
  in `<home>/local/node.json`, readable by your user alone.
- It does not tell machines apart: every machine with the key is you.
- It does not find the other machine by itself. You give it a name or an
  address once.
- It is for a network you trust enough to be on. Across the internet, put
  it behind a private network of your own (WireGuard, Tailscale).

## One machine

Nothing here is needed for a single machine. A memory that was never told
about another keeps to itself, and `nahuali node forget` makes it so again.
