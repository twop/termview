# termview

A background daemon that shows a single floating, undecorated, Nord-themed
terminal window backed by real PTY sessions — a toggleable, persistent file
manager (yazi) alongside one-shot pickers (fzf) that report their result and
disappear. Driven entirely by CLI calls, e.g. from Hammerspoon.

macOS only.

## Build

```
cargo build --release
```

The binary is `target/release/termview`. The first `open` call auto-spawns
the background daemon; no separate step needed.

## Usage

```
# Long-running session, reused across shows/hides until its command changes
termview open "yazi ~/projects" --workspace files --persistent

# One-shot picker: window/session close automatically when fzf exits, and
# the selected line is reported back via `open <url>`
termview open "fzf" --workspace picker \
  --report "hammerspoon://stdout/?key={stdout}"

# Stop the daemon and all sessions
termview quit
```

`--workspace` is the session's identity: reopening the same workspace with
the same command reuses the running session; a different command kills the
old one and starts fresh. Sessions are **ephemeral by default** (Cmd+W kills
them); pass `--persistent` for ones that should keep running in the
background when hidden.

Other `open` flags: `--width`/`--height` (override for this call; default is
centered 800×400), `--cwd` (tilde-expanded).

## Closing a window

- **Cmd+W**: ephemeral session → killed; persistent session → just hidden.
- **Tray icon**: lists persistent sessions (click to show), a "Close
  Session" submenu to kill one, and "Quit" to stop everything.

## Shell environment

At daemon startup, termview runs the resolved shell once as a login shell
(`<shell> -l -c "/usr/bin/env"`) and merges the result into the daemon's own
process environment — every session spawned afterward inherits that, not a
fresh shell environment on each `open`. This means any environment variable
a program run inside termview depends on must actually be *exported*
somewhere in that shell's own startup files (`~/.zshrc`/`~/.zshenv` for
zsh) — termview has no way to provide anything the shell itself wouldn't
already have.

Worth knowing specifically: macOS doesn't set the XDG Base Directory
variables (`XDG_CONFIG_HOME` etc.) by default. Several CLI tools — nushell
included — use `$XDG_CONFIG_HOME` rather than any macOS-native convention
to find their config directory, so without it exported, such a tool can
silently fall back to the wrong (often empty) config location instead of
erroring. If a program's config/theme/aliases don't seem to apply when run
inside termview but work fine in a regular terminal, check whether it
depends on an env var your shell doesn't actually export.
