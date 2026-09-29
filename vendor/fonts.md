# Vendored fonts

`JetBrainsMonoNerdFont-{Regular,Bold,Italic,BoldItalic}.ttf` — the four static weight/style instances of JetBrains Mono, patched with Nerd Font icon glyphs (Powerline/devicons/etc., used by shell prompts, yazi, etc.).

## Why

egui has no font-weight/style synthesis (`egui::FontId` is just `{size, family}`) — the only way to render true bold/italic is to load separate static font files, each registered as its own named font family. A variable-weight instance of JetBrains Mono exists, but epaint has no support for selecting an arbitrary weight axis at render time, so static per-style files are required (same approach the sibling `shelv` project uses for its Inter fonts).

Regular alone was already in use (loaded from an absolute path outside the repo); Bold/Italic/BoldItalic are added here so termview can render them distinctly instead of falling back to Regular for everything.

## License

JetBrains Mono and the Nerd Fonts patching are both licensed under the SIL Open Font License 1.1 (https://scripts.sil.org/OFL).
