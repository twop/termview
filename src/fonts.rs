// egui_term has no line-height/row-spacing knob of its own, and box-drawing/border
// glyphs are fixed-height (rendered at the font's native size, not stretched to the
// cell), so any attempt to inflate row height independent of glyph size — a patched
// font lineGap, a hypothetical egui_term line-height field — leaves gaps in border
// characters since only empty space is added, not glyph height. Plain system font
// gives correct rendering at the cost of cramped rows, matching term_kick's choice.
// Bold/italic/bold-italic are separate static instances of the same family (see
// vendor/fonts.md) — egui has no font-weight/style synthesis of its own.

pub const JETBRAINS_MONO_NERD: &str = "jetbrains-mono-nerd";
pub const JETBRAINS_MONO_NERD_BOLD: &str = "jetbrains-mono-nerd-bold";
pub const JETBRAINS_MONO_NERD_ITALIC: &str = "jetbrains-mono-nerd-italic";
pub const JETBRAINS_MONO_NERD_BOLD_ITALIC: &str = "jetbrains-mono-nerd-bold-italic";

const REGULAR: &[u8] = include_bytes!("../vendor/fonts/JetBrainsMonoNerdFont-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../vendor/fonts/JetBrainsMonoNerdFont-Bold.ttf");
const ITALIC: &[u8] = include_bytes!("../vendor/fonts/JetBrainsMonoNerdFont-Italic.ttf");
const BOLD_ITALIC: &[u8] = include_bytes!("../vendor/fonts/JetBrainsMonoNerdFont-BoldItalic.ttf");

pub fn register_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    for (name, bytes) in [
        (JETBRAINS_MONO_NERD, REGULAR),
        (JETBRAINS_MONO_NERD_BOLD, BOLD),
        (JETBRAINS_MONO_NERD_ITALIC, ITALIC),
        (JETBRAINS_MONO_NERD_BOLD_ITALIC, BOLD_ITALIC),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), egui::FontData::from_static(bytes).into());
        fonts
            .families
            .entry(egui::FontFamily::Name(name.into()))
            .or_default()
            .push(name.to_owned());
    }

    // Regular also stays the app's default Monospace fallback, unchanged
    // from before, for anything else in the app that requests it.
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, JETBRAINS_MONO_NERD.to_owned());

    ctx.set_fonts(fonts);
}
