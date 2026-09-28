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
