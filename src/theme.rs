use crate::nord::Nord;
use egui::Color32;
use egui_term::{ColorPalette, TerminalTheme};

pub struct SemanticColors {
    pub background: Color32,
    pub border: Color32,
}

impl SemanticColors {
    pub fn nord() -> Self {
        Self {
            background: Nord::NORD0,
            border: Nord::NORD2,
        }
    }

    pub fn terminal_theme(&self) -> TerminalTheme {
        TerminalTheme::new(Box::new(ColorPalette {
            foreground: hex(Nord::NORD4),
            background: hex(Nord::NORD0),
            black: hex(Nord::NORD1),
            red: hex(Nord::NORD11),
            green: hex(Nord::NORD14),
            yellow: hex(Nord::NORD13),
            blue: hex(Nord::NORD9),
            magenta: hex(Nord::NORD15),
            cyan: hex(Nord::NORD8),
            white: hex(Nord::NORD5),
            bright_black: hex(Nord::NORD3),
            bright_red: hex(Nord::NORD11),
            bright_green: hex(Nord::NORD14),
            bright_yellow: hex(Nord::NORD13),
            bright_blue: hex(Nord::NORD9),
            bright_magenta: hex(Nord::NORD15),
            bright_cyan: hex(Nord::NORD7),
            bright_white: hex(Nord::NORD6),
            bright_foreground: None,
            dim_foreground: hex(Nord::NORD4),
            dim_black: hex(Nord::NORD0),
            dim_red: hex(Nord::NORD11),
            dim_green: hex(Nord::NORD14),
            dim_yellow: hex(Nord::NORD13),
            dim_blue: hex(Nord::NORD9),
            dim_magenta: hex(Nord::NORD15),
            dim_cyan: hex(Nord::NORD8),
            dim_white: hex(Nord::NORD4),
        }))
    }
}

fn hex(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}
