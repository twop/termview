use egui::Color32;
use egui_term::{ColorPalette, TerminalTheme};
use serde::Deserialize;

/// Mirrors `egui_term::ColorPalette` field-for-field so theme TOML files can
/// be deserialized without patching the vendored crate (that crate's patch
/// surface stays limited to the earlier Kitty-protocol work).
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct SerdeColorPalette {
    foreground: String,
    background: String,
    black: String,
    red: String,
    green: String,
    yellow: String,
    blue: String,
    magenta: String,
    cyan: String,
    white: String,
    bright_black: String,
    bright_red: String,
    bright_green: String,
    bright_yellow: String,
    bright_blue: String,
    bright_magenta: String,
    bright_cyan: String,
    bright_white: String,
    bright_foreground: Option<String>,
    dim_foreground: String,
    dim_black: String,
    dim_red: String,
    dim_green: String,
    dim_yellow: String,
    dim_blue: String,
    dim_magenta: String,
    dim_cyan: String,
    dim_white: String,
}

impl Default for SerdeColorPalette {
    fn default() -> Self {
        let d = ColorPalette::default();
        Self {
            foreground: d.foreground,
            background: d.background,
            black: d.black,
            red: d.red,
            green: d.green,
            yellow: d.yellow,
            blue: d.blue,
            magenta: d.magenta,
            cyan: d.cyan,
            white: d.white,
            bright_black: d.bright_black,
            bright_red: d.bright_red,
            bright_green: d.bright_green,
            bright_yellow: d.bright_yellow,
            bright_blue: d.bright_blue,
            bright_magenta: d.bright_magenta,
            bright_cyan: d.bright_cyan,
            bright_white: d.bright_white,
            bright_foreground: d.bright_foreground,
            dim_foreground: d.dim_foreground,
            dim_black: d.dim_black,
            dim_red: d.dim_red,
            dim_green: d.dim_green,
            dim_yellow: d.dim_yellow,
            dim_blue: d.dim_blue,
            dim_magenta: d.dim_magenta,
            dim_cyan: d.dim_cyan,
            dim_white: d.dim_white,
        }
    }
}

impl From<SerdeColorPalette> for ColorPalette {
    fn from(s: SerdeColorPalette) -> Self {
        ColorPalette {
            foreground: s.foreground,
            background: s.background,
            black: s.black,
            red: s.red,
            green: s.green,
            yellow: s.yellow,
            blue: s.blue,
            magenta: s.magenta,
            cyan: s.cyan,
            white: s.white,
            bright_black: s.bright_black,
            bright_red: s.bright_red,
            bright_green: s.bright_green,
            bright_yellow: s.bright_yellow,
            bright_blue: s.bright_blue,
            bright_magenta: s.bright_magenta,
            bright_cyan: s.bright_cyan,
            bright_white: s.bright_white,
            bright_foreground: s.bright_foreground,
            dim_foreground: s.dim_foreground,
            dim_black: s.dim_black,
            dim_red: s.dim_red,
            dim_green: s.dim_green,
            dim_yellow: s.dim_yellow,
            dim_blue: s.dim_blue,
            dim_magenta: s.dim_magenta,
            dim_cyan: s.dim_cyan,
            dim_white: s.dim_white,
        }
    }
}

pub struct SemanticColors {
    pub background: Color32,
    pub border: Color32,
}

impl SemanticColors {
    // border uses `bright_black`: Nord's own docs describe that shade's role
    // as UI "indent/wrap guide" elements - a border role - while `black`
    // sits too close to `background` to read as a border. ColorPalette has
    // no dedicated "border" field; this only affects window chrome, not
    // terminal rendering.
    pub fn from_palette(palette: &ColorPalette) -> Self {
        Self {
            background: hex_to_color32(&palette.background)
                .unwrap_or(Color32::from_rgb(24, 24, 24)),
            border: hex_to_color32(&palette.bright_black)
                .unwrap_or(Color32::from_rgb(64, 64, 64)),
        }
    }
}

/// Reads+parses one theme file, falling back to `nord-night` (not a generic
/// default) on any I/O or parse error - nord-night is guaranteed present,
/// since it's re-seeded on every startup.
pub fn parse_theme_file(path: &std::path::Path) -> ColorPalette {
    match std::fs::read_to_string(path) {
        Ok(contents) => match toml::from_str::<SerdeColorPalette>(&contents) {
            Ok(parsed) => return parsed.into(),
            Err(err) => {
                eprintln!("termview: failed to parse theme {}: {err}", path.display())
            }
        },
        Err(err) => eprintln!("termview: failed to read theme {}: {err}", path.display()),
    }
    fallback_palette()
}

/// The guaranteed-present fallback: nord-night's own seed content, parsed
/// directly (not a file-lookup - avoids infinite fallback recursion if
/// nord-night's file is itself somehow broken, in which case this still
/// degrades to `ColorPalette::default()` via `SerdeColorPalette`'s own
/// container-level default).
fn fallback_palette() -> ColorPalette {
    const NORD_NIGHT: &str = include_str!("../assets/themes/nord-night.toml");
    toml::from_str::<SerdeColorPalette>(NORD_NIGHT)
        .unwrap_or_default()
        .into()
}

pub fn load_theme(name: &str) -> ColorPalette {
    let path = crate::paths::theme_file_path(name);
    if path.exists() {
        parse_theme_file(&path)
    } else {
        eprintln!("termview: theme {name:?} not found, falling back to nord-night");
        fallback_palette()
    }
}

pub struct LoadedTheme {
    pub palette: ColorPalette,
    pub semantic: SemanticColors,
}

impl LoadedTheme {
    pub fn load(name: &str) -> Self {
        let palette = load_theme(name);
        let semantic = SemanticColors::from_palette(&palette);
        Self { palette, semantic }
    }

    pub fn terminal_theme(&self) -> TerminalTheme {
        TerminalTheme::new(Box::new(self.palette.clone()))
    }
}

pub fn hex_to_color32(hex: &str) -> Option<Color32> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color32::from_rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_to_color32_round_trips() {
        assert_eq!(hex_to_color32("#2e3440"), Some(Color32::from_rgb(46, 52, 64)));
    }

    #[test]
    fn hex_to_color32_rejects_malformed_input() {
        assert_eq!(hex_to_color32("2e3440"), None); // missing '#'
        assert_eq!(hex_to_color32("#2e344"), None); // too short
        assert_eq!(hex_to_color32("#2e34400"), None); // too long
        assert_eq!(hex_to_color32("#zzzzzz"), None); // non-hex
    }

    #[test]
    fn partial_theme_file_falls_back_to_defaults_for_missing_fields() {
        let parsed: SerdeColorPalette = toml::from_str("background = \"#000000\"").unwrap();
        let palette: ColorPalette = parsed.into();
        let default = ColorPalette::default();
        assert_eq!(palette.background, "#000000");
        assert_eq!(palette.foreground, default.foreground);
        assert_eq!(palette.red, default.red);
        assert_eq!(palette.bright_white, default.bright_white);
        assert_eq!(palette.bright_foreground, default.bright_foreground);
    }

    #[test]
    fn missing_theme_falls_back_to_nord_night() {
        let missing = load_theme("this-theme-does-not-exist");
        let nord_night = fallback_palette();
        assert_eq!(missing.background, nord_night.background);
        assert_eq!(missing.foreground, nord_night.foreground);
    }
}
