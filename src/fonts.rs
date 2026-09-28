// egui_term has no line-height/row-spacing knob of its own, and box-drawing/border
// glyphs are fixed-height (rendered at the font's native size, not stretched to the
// cell), so any attempt to inflate row height independent of glyph size — a patched
// font lineGap, a hypothetical egui_term line-height field — leaves gaps in border
// characters since only empty space is added, not glyph height. Plain system font
// gives correct rendering at the cost of cramped rows, matching term_kick's choice.
const NERD_FONT_BYTES: &[u8] =
    include_bytes!("/Users/simonkorzunov/Library/Fonts/JetBrainsMonoNerdFont-Regular.ttf");

pub fn register_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "jetbrains-mono-nerd".to_owned(),
        egui::FontData::from_static(NERD_FONT_BYTES).into(),
    );

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "jetbrains-mono-nerd".to_owned());

    ctx.set_fonts(fonts);
}
