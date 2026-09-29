# Vendored fonts

`JetBrainsMonoNerdFont-{Thin,Medium,ThinItalic,MediumItalic}.ttf` — four static weight/style instances of JetBrains Mono, patched with Nerd Font icon glyphs (Powerline/devicons/etc., used by shell prompts, yazi, etc.).

## Why

egui has no font-weight/style synthesis (`egui::FontId` is just `{size, family}`) — the only way to render true bold/italic is to load separate static font files, each registered as its own named font family. A variable-weight instance of JetBrains Mono exists, but epaint has no support for selecting an arbitrary weight axis at render time, so static per-style files are required (same approach the sibling `shelv` project uses for its Inter fonts).

**Weight mapping matches this user's Alacritty config exactly** (see `src/fonts.rs`):
```
normal      = { family = "JetBrainsMono Nerd Font", style = "Thin" }
bold        = { family = "JetBrainsMono Nerd Font", style = "Medium" }
italic      = { family = "JetBrainsMono Nerd Font", style = "Thin Italic" }
bold_italic = { family = "JetBrainsMono Nerd Font", style = "Medium Italic" }
```
i.e. `Thin` fills the "normal" (non-bold, non-italic) role, `Medium` fills "bold", `ThinItalic` fills "italic", and `MediumItalic` fills "bold-italic" — none of the four filenames contain the words "Bold" despite one of them filling that role.

## License

JetBrains Mono and the Nerd Fonts patching are both licensed under the SIL Open Font License 1.1 (https://scripts.sil.org/OFL).
