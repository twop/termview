use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub padding: f32,
    pub corner_radius: u8,
    pub command_palette_shortcut: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "nord-night".to_string(),
            padding: 4.0,
            corner_radius: 14,
            command_palette_shortcut: "cmd+shift+p".to_string(),
        }
    }
}

const BUNDLED_THEMES: &[(&str, &str)] = &[
    ("nord-night", include_str!("../assets/themes/nord-night.toml")),
    ("nord", include_str!("../assets/themes/nord.toml")),
    ("dracula", include_str!("../assets/themes/dracula.toml")),
    (
        "catppuccin_mocha",
        include_str!("../assets/themes/catppuccin_mocha.toml"),
    ),
    (
        "catppuccin_latte",
        include_str!("../assets/themes/catppuccin_latte.toml"),
    ),
    (
        "catppuccin_frappe",
        include_str!("../assets/themes/catppuccin_frappe.toml"),
    ),
    (
        "catppuccin_macchiato",
        include_str!("../assets/themes/catppuccin_macchiato.toml"),
    ),
    ("gruvbox", include_str!("../assets/themes/gruvbox.toml")),
];

/// Writes each bundled seed theme to disk if (and only if) it's not already
/// there — never overwrites a user's edited copy. Runs on every startup, not
/// just first-ever run, so deleting one theme file (while keeping others /
/// config.toml) still self-heals.
fn seed_bundled_themes() {
    if std::fs::create_dir_all(crate::paths::themes_dir()).is_err() {
        eprintln!("termview: failed to create themes dir");
        return;
    }
    for (name, contents) in BUNDLED_THEMES {
        let path = crate::paths::theme_file_path(name);
        if !path.exists() && let Err(err) = std::fs::write(&path, contents) {
            eprintln!("termview: failed to seed theme {name}: {err}");
        }
    }
}

impl Config {
    /// Reads `~/.config/termview/config.toml`, bootstrapping it (and the
    /// bundled seed themes) on a fresh install. Never panics: any I/O or
    /// parse failure is reported to stderr and this falls back to
    /// `Config::default()` in memory, leaving a malformed on-disk file
    /// untouched so the user can fix it.
    pub fn load() -> Self {
        let path = crate::paths::config_file_path();
        let config = if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(raw) => match toml::from_str::<Config>(&raw) {
                    Ok(config) => config,
                    Err(err) => {
                        eprintln!(
                            "termview: failed to parse {}: {err}; using defaults",
                            path.display()
                        );
                        Config::default()
                    }
                },
                Err(err) => {
                    eprintln!(
                        "termview: failed to read {}: {err}; using defaults",
                        path.display()
                    );
                    Config::default()
                }
            }
        } else {
            let config = Config::default();
            if let Err(err) = config.save() {
                eprintln!("termview: failed to write default config: {err}");
            }
            config
        };

        seed_bundled_themes();
        config
    }

    pub fn save(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(crate::paths::config_dir())?;
        let raw = toml::to_string_pretty(self).expect("Config always serializes");
        std::fs::write(crate::paths::config_file_path(), raw)
    }

    pub fn parsed_palette_shortcut(&self) -> KeyCombo {
        KeyCombo::parse(&self.command_palette_shortcut)
    }
}

// ---- configurable shortcut parsing ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyCombo {
    pub key: egui::Key,
    pub mac_cmd: bool,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl KeyCombo {
    fn default_palette_shortcut() -> Self {
        Self {
            key: egui::Key::P,
            mac_cmd: true,
            shift: true,
            ctrl: false,
            alt: false,
        }
    }

    pub fn matches(&self, key: egui::Key, modifiers: &egui::Modifiers) -> bool {
        key == self.key
            && modifiers.mac_cmd == self.mac_cmd
            && modifiers.shift == self.shift
            && modifiers.ctrl == self.ctrl
            && modifiers.alt == self.alt
    }

    pub fn parse(spec: &str) -> Self {
        Self::try_parse(spec).unwrap_or_else(|| {
            eprintln!(
                "termview: invalid command_palette_shortcut {spec:?}; falling back to cmd+shift+p"
            );
            Self::default_palette_shortcut()
        })
    }

    fn try_parse(spec: &str) -> Option<Self> {
        let mut combo = Self {
            key: egui::Key::P,
            mac_cmd: false,
            shift: false,
            ctrl: false,
            alt: false,
        };
        let mut key_set = false;
        for part in spec.split('+') {
            let part = part.trim().to_ascii_lowercase();
            match part.as_str() {
                "" => return None,
                "cmd" | "command" | "super" | "meta" => combo.mac_cmd = true,
                "shift" => combo.shift = true,
                "ctrl" | "control" => combo.ctrl = true,
                "alt" | "option" => combo.alt = true,
                other => {
                    if key_set {
                        return None; // more than one non-modifier token
                    }
                    combo.key = parse_key(other)?;
                    key_set = true;
                }
            }
        }
        key_set.then_some(combo)
    }
}

fn parse_key(token: &str) -> Option<egui::Key> {
    // Single chars: from_name accepts either case directly ("p"/"P").
    if token.chars().count() == 1 {
        return egui::Key::from_name(token);
    }
    // Function keys: from_name needs exact "F1".."F35" (capital F).
    if let Some(digits) = token.strip_prefix('f')
        && !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
    {
        return egui::Key::from_name(&format!("F{digits}"));
    }
    // Other named keys: from_name's spellings aren't a simple
    // uppercase-first rule (e.g. "PageUp"), so map explicitly.
    match token {
        "tab" => Some(egui::Key::Tab),
        "enter" | "return" => Some(egui::Key::Enter),
        "escape" | "esc" => Some(egui::Key::Escape),
        "space" => Some(egui::Key::Space),
        "backspace" => Some(egui::Key::Backspace),
        "delete" => Some(egui::Key::Delete),
        "home" => Some(egui::Key::Home),
        "end" => Some(egui::Key::End),
        "pageup" => Some(egui::Key::PageUp),
        "pagedown" => Some(egui::Key::PageDown),
        "up" => Some(egui::Key::ArrowUp),
        "down" => Some(egui::Key::ArrowDown),
        "left" => Some(egui::Key::ArrowLeft),
        "right" => Some(egui::Key::ArrowRight),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_parses_from_its_own_string() {
        assert_eq!(
            KeyCombo::parse("cmd+shift+p"),
            KeyCombo::default_palette_shortcut()
        );
    }

    #[test]
    fn parsing_is_case_insensitive() {
        assert_eq!(
            KeyCombo::parse("Cmd+Shift+P"),
            KeyCombo::default_palette_shortcut()
        );
    }

    #[test]
    fn parses_ctrl_alt_and_named_keys() {
        let combo = KeyCombo::parse("ctrl+alt+t");
        assert_eq!(combo.key, egui::Key::T);
        assert!(combo.ctrl && combo.alt && !combo.shift && !combo.mac_cmd);

        let combo = KeyCombo::parse("ctrl+escape");
        assert_eq!(combo.key, egui::Key::Escape);

        let combo = KeyCombo::parse("cmd+pageup");
        assert_eq!(combo.key, egui::Key::PageUp);

        let combo = KeyCombo::parse("cmd+f5");
        assert_eq!(combo.key, egui::Key::F5);
    }

    #[test]
    fn malformed_specs_fall_back_to_default() {
        for spec in ["", "cmd+", "banana", "cmd+shift+p+extra", "cmd+p+k"] {
            assert_eq!(
                KeyCombo::parse(spec),
                KeyCombo::default_palette_shortcut(),
                "spec {spec:?} should fall back to default"
            );
        }
    }

    #[test]
    fn load_bootstraps_config_and_all_bundled_themes_on_a_fresh_home() {
        let scratch_home = std::env::temp_dir().join(format!(
            "termview-config-bootstrap-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&scratch_home).unwrap();
        let original_home = std::env::var("HOME").ok();
        // SAFETY: this test process doesn't touch HOME concurrently from
        // other threads (no other test reads/writes it), and the original
        // value is restored before returning.
        unsafe {
            std::env::set_var("HOME", &scratch_home);
        }

        let config = Config::load();
        assert_eq!(config, Config::default());
        assert!(crate::paths::config_file_path().exists());
        for (name, _) in BUNDLED_THEMES {
            assert!(
                crate::paths::theme_file_path(name).exists(),
                "expected {name} to be seeded"
            );
        }

        unsafe {
            match &original_home {
                Some(home) => std::env::set_var("HOME", home),
                None => std::env::remove_var("HOME"),
            }
        }
        std::fs::remove_dir_all(&scratch_home).unwrap();
    }

    #[test]
    fn config_partial_toml_falls_back_to_defaults_for_missing_fields() {
        let config: Config = toml::from_str("theme = \"dracula\"").unwrap();
        assert_eq!(config.theme, "dracula");
        assert_eq!(config.padding, Config::default().padding);
        assert_eq!(config.corner_radius, Config::default().corner_radius);
        assert_eq!(
            config.command_palette_shortcut,
            Config::default().command_palette_shortcut
        );
    }
}
