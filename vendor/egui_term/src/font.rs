use egui::{Context, FontId};

use crate::types::Size;
use alacritty_terminal::term::cell;

#[derive(Debug, Clone)]
pub struct FontSettings {
    pub font_type: FontId,
    pub bold: FontId,
    pub italic: FontId,
    pub bold_italic: FontId,
}

impl Default for FontSettings {
    fn default() -> Self {
        let font_type = FontId::monospace(14.0);
        Self {
            font_type: font_type.clone(),
            bold: font_type.clone(),
            italic: font_type.clone(),
            bold_italic: font_type,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerminalFont {
    font_type: FontId,
    bold: FontId,
    italic: FontId,
    bold_italic: FontId,
}

impl Default for TerminalFont {
    fn default() -> Self {
        Self::new(FontSettings::default())
    }
}

impl TerminalFont {
    pub fn new(settings: FontSettings) -> Self {
        Self {
            font_type: settings.font_type,
            bold: settings.bold,
            italic: settings.italic,
            bold_italic: settings.bold_italic,
        }
    }

    pub fn font_type(&self) -> FontId {
        self.font_type.clone()
    }

    /// Picks the font for a cell's bold/italic attributes. `DIM_BOLD` (dim +
    /// bold combined) already has the `BOLD` bit set, so checking `BOLD`
    /// alone naturally covers it too - dimming only affects color (handled
    /// elsewhere), not font selection. (`intersects(BOLD | DIM_BOLD)` would
    /// be wrong here: `DIM_BOLD`'s bits overlap plain `DIM`, so that would
    /// incorrectly treat dim-only text as bold.)
    pub fn font_for_flags(&self, flags: cell::Flags) -> FontId {
        let bold = flags.contains(cell::Flags::BOLD);
        let italic = flags.contains(cell::Flags::ITALIC);
        match (bold, italic) {
            (true, true) => self.bold_italic.clone(),
            (true, false) => self.bold.clone(),
            (false, true) => self.italic.clone(),
            (false, false) => self.font_type.clone(),
        }
    }

    pub fn font_measure(&self, ctx: &Context) -> Size {
        // Measured off the regular weight: a real monospace family's
        // bold/italic static instances share the same advance width, so
        // cell sizing stays correct regardless of which style is used.
        let (width, height) = ctx.fonts_mut(|f| {
            (
                f.glyph_width(&self.font_type, 'm'),
                f.row_height(&self.font_type),
            )
        });

        Size::new(width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> FontSettings {
        FontSettings {
            font_type: FontId::monospace(1.0),
            bold: FontId::monospace(2.0),
            italic: FontId::monospace(3.0),
            bold_italic: FontId::monospace(4.0),
        }
    }

    #[test]
    fn plain_flags_use_regular() {
        let font = TerminalFont::new(settings());
        assert_eq!(font.font_for_flags(cell::Flags::empty()), font.font_type);
    }

    #[test]
    fn bold_flag_uses_bold() {
        let font = TerminalFont::new(settings());
        assert_eq!(font.font_for_flags(cell::Flags::BOLD), font.bold);
    }

    #[test]
    fn dim_bold_flag_uses_bold() {
        let font = TerminalFont::new(settings());
        assert_eq!(font.font_for_flags(cell::Flags::DIM_BOLD), font.bold);
    }

    #[test]
    fn italic_flag_uses_italic() {
        let font = TerminalFont::new(settings());
        assert_eq!(font.font_for_flags(cell::Flags::ITALIC), font.italic);
    }

    #[test]
    fn bold_italic_flag_uses_bold_italic() {
        let font = TerminalFont::new(settings());
        assert_eq!(font.font_for_flags(cell::Flags::BOLD_ITALIC), font.bold_italic);
    }

    #[test]
    fn dim_alone_does_not_trigger_bold() {
        let font = TerminalFont::new(settings());
        assert_eq!(font.font_for_flags(cell::Flags::DIM), font.font_type);
    }
}
