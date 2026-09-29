# Vendored egui_term

Local patched copy of `github.com/kemokempo/egui_term` @ `31bbc7ab8503c9518fcee5717cfa29011e59f451` (branch `main`). Vendored because that fork is read-only to this project; patched via `[patch]` in the root `Cargo.toml`.

## Why

Cmd+S (and other Cmd-modified keys) did nothing inside apps like Helix. Root cause: this crate never enabled the Kitty keyboard protocol, so kitty-aware apps got no response to their `CSI ?u` capability query and fell back to a legacy key-binding table that has no entry for Cmd+S — the key event was silently dropped.

## Changes

- `src/backend/mod.rs`: `Config::kitty_keyboard` set to `true` (was `false`). Activates the mode push/pop/query handling that `alacritty_terminal` already implements but keeps disabled by default.
- `src/view.rs`: added a Kitty keyboard protocol key encoder (`process_kitty_keyboard_key` + helpers), used only once a terminal app negotiates kitty mode. Encodes keys as `CSI code;modifiers[:event-type]u` (e.g. Cmd+S → `\x1b[115;9u`) instead of dropping unbound combos. Legacy path is unchanged and still used whenever kitty mode isn't negotiated. Also stopped unconditionally discarding key-release/repeat events (now forwarded when the app requests event-type reporting).

Not implemented: `REPORT_ALTERNATE_KEYS` / `REPORT_ASSOCIATED_TEXT` / `REPORT_ALL_KEYS_AS_ESC` submodes — not needed by target apps (Helix negotiates disambiguation + event types only).
