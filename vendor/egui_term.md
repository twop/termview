# Vendored egui_term

Local patched copy of `github.com/kemokempo/egui_term` @ `31bbc7ab8503c9518fcee5717cfa29011e59f451` (branch `main`). Vendored because that fork is read-only to this project; patched via `[patch]` in the root `Cargo.toml`.

## Why

Cmd+S (and other Cmd-modified keys) did nothing inside apps like Helix. Root cause: this crate never enabled the Kitty keyboard protocol, so kitty-aware apps got no response to their `CSI ?u` capability query and fell back to a legacy key-binding table that has no entry for Cmd+S — the key event was silently dropped.

## Changes

- `src/backend/mod.rs`: `Config::kitty_keyboard` set to `true` (was `false`). Activates the mode push/pop/query handling that `alacritty_terminal` already implements but keeps disabled by default.
- `src/view.rs`: added a Kitty keyboard protocol key encoder (`process_kitty_keyboard_key` + helpers), used only once a terminal app negotiates kitty mode. Encodes keys as `CSI code;modifiers[:event-type]u` (e.g. Cmd+S → `\x1b[115;9u`) instead of dropping unbound combos. Legacy path is unchanged and still used whenever kitty mode isn't negotiated. Also stopped unconditionally discarding key-release/repeat events (now forwarded when the app requests event-type reporting).

Not implemented: `REPORT_ALTERNATE_KEYS` / `REPORT_ASSOCIATED_TEXT` / `REPORT_ALL_KEYS_AS_ESC` submodes — not needed by target apps (Helix negotiates disambiguation + event types only).

## Changes (2)

Bold/italic text (Helix UI/markup, `ls` bold, etc.) all rendered identically — this crate only ever drew every cell with one single font, regardless of the cell's bold/italic attributes.

- `src/font.rs`: `FontSettings`/`TerminalFont` gained `bold`/`italic`/`bold_italic: FontId` fields alongside the existing `font_type` (regular), plus `TerminalFont::font_for_flags(cell::Flags) -> FontId` to pick the right one. `font_measure` still measures off the regular weight (a real monospace family's other static instances share the same advance width).
- `src/view.rs`: the per-cell text-draw call now uses `self.font.font_for_flags(flags)` instead of the single `self.font.font_type()`.
- Requires the caller to supply four separate static font-file-backed `FontId`s (see `vendor/fonts.md` + `src/fonts.rs`) — egui has no font-weight/style synthesis of its own.
