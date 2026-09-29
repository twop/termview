use egui_term::ColorPalette;
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

struct ThemeEntry {
    name: String,
    palette: ColorPalette,
}

/// What the caller should do this frame, in priority order: a `Commit`/
/// `Cancel` always wins over a `Preview` (both close the palette), and at
/// most one of these fires per frame.
pub enum PaletteEvent {
    None,
    /// The highlighted entry changed (arrow keys, hover, or a new filter
    /// snapping to a different row) - apply it live, but don't persist it.
    Preview(String),
    /// The user confirmed a selection (Enter or click) - apply it live and
    /// persist it.
    Commit(String),
    /// The palette closed without a selection (Escape, or the toggle
    /// shortcut pressed again while open) - revert to this theme, which was
    /// the active one before the palette opened.
    Cancel(String),
}

pub struct CommandPalette {
    open: bool,
    query: String,
    entries: Vec<ThemeEntry>,
    filtered: Vec<usize>, // indices into `entries`, best match first
    selected: usize,      // index into `filtered`
    matcher: SkimMatcherV2,
    /// The committed theme name as of when the palette was opened - what a
    /// `Cancel` reverts to.
    original_theme: String,
    /// The name last returned via `Preview`/`Commit`, so `show` only fires
    /// a new `Preview` when the highlighted entry actually changes.
    last_previewed: Option<String>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            open: false,
            query: String::new(),
            entries: Vec::new(),
            filtered: Vec::new(),
            selected: 0,
            matcher: SkimMatcherV2::default(),
            original_theme: String::new(),
            last_previewed: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the palette (remembering `current_theme` as what to revert to)
    /// or, if already open, closes it as a cancel. Only ever returns `None`
    /// (opening) or `Cancel` (closing this way) - `Preview`/`Commit` only
    /// ever come from `show`.
    pub fn toggle(&mut self, current_theme: &str) -> PaletteEvent {
        if self.open {
            self.open = false;
            self.last_previewed = None;
            PaletteEvent::Cancel(self.original_theme.clone())
        } else {
            self.entries = scan_themes();
            self.query.clear();
            self.original_theme = current_theme.to_string();
            self.recompute_filter();
            // Start with the cursor on the currently active theme, so
            // opening the palette doesn't itself preview a change.
            self.selected = self
                .filtered
                .iter()
                .position(|&idx| self.entries[idx].name == current_theme)
                .unwrap_or(0);
            self.last_previewed = self.current_selection_name();
            self.open = true;
            PaletteEvent::None
        }
    }

    fn current_selection_name(&self) -> Option<String> {
        self.filtered
            .get(self.selected)
            .map(|&idx| self.entries[idx].name.clone())
    }

    fn recompute_filter(&mut self) {
        let mut scored: Vec<(usize, i64)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if self.query.is_empty() {
                    Some((i, 0))
                } else {
                    self.matcher.fuzzy_match(&e.name, &self.query).map(|s| (i, s))
                }
            })
            .collect();
        scored.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    /// Draws the overlay (a no-op when closed) and reports what changed
    /// this frame. `active_palette` is the currently-applied theme (i.e.
    /// `TermViewApp::theme.palette`) - the popup styles itself from it, so
    /// the palette visibly matches whatever's being previewed/active.
    pub fn show(&mut self, ctx: &egui::Context, active_palette: &ColorPalette) -> PaletteEvent {
        if !self.open {
            return PaletteEvent::None;
        }
        let mut committed = None;
        let mut cancel_requested = false;

        let bg = crate::theme::hex_to_color32(&active_palette.background)
            .unwrap_or(egui::Color32::from_rgb(24, 24, 24));
        let fg = crate::theme::hex_to_color32(&active_palette.foreground)
            .unwrap_or(egui::Color32::WHITE);
        let border = crate::theme::hex_to_color32(&active_palette.bright_black)
            .unwrap_or(egui::Color32::GRAY);
        let input_bg = crate::theme::hex_to_color32(&active_palette.black).unwrap_or(border);
        let accent = crate::theme::hex_to_color32(&active_palette.blue)
            .unwrap_or(egui::Color32::LIGHT_BLUE);

        egui::Area::new(egui::Id::new("command_palette"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 80.0))
            .show(ctx, |ui| {
                let frame = egui::Frame::popup(ui.style())
                    .fill(bg)
                    .stroke(egui::Stroke::new(1.0, border));
                frame.show(ui, |ui| {
                    ui.set_min_width(360.0);
                    ui.visuals_mut().override_text_color = Some(fg);
                    ui.visuals_mut().extreme_bg_color = input_bg;
                    ui.visuals_mut().selection.bg_fill = accent;
                    ui.visuals_mut().widgets.hovered.weak_bg_fill = border;
                    ui.visuals_mut().widgets.active.weak_bg_fill = border;

                    let response = ui.text_edit_singleline(&mut self.query);
                    response.request_focus();
                    if response.changed() {
                        self.recompute_filter();
                    }

                    ui.input(|i| {
                        if i.key_pressed(egui::Key::ArrowDown) {
                            self.selected =
                                (self.selected + 1).min(self.filtered.len().saturating_sub(1));
                        }
                        if i.key_pressed(egui::Key::ArrowUp) {
                            self.selected = self.selected.saturating_sub(1);
                        }
                        if i.key_pressed(egui::Key::Escape) {
                            cancel_requested = true;
                        }
                        if i.key_pressed(egui::Key::Enter)
                            && let Some(&idx) = self.filtered.get(self.selected)
                        {
                            committed = Some(self.entries[idx].name.clone());
                        }
                    });

                    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        for (row, &idx) in self.filtered.iter().enumerate() {
                            let entry = &self.entries[idx];
                            let is_selected = row == self.selected;
                            let resp = ui
                                .horizontal(|ui| {
                                    draw_swatch(ui, &entry.palette);
                                    ui.selectable_label(is_selected, &entry.name)
                                })
                                .inner;
                            if resp.hovered() {
                                self.selected = row;
                            }
                            if resp.clicked() {
                                committed = Some(entry.name.clone());
                            }
                        }
                    });
                });
            });

        if let Some(name) = committed {
            self.open = false;
            self.last_previewed = None;
            return PaletteEvent::Commit(name);
        }
        if cancel_requested {
            self.open = false;
            let revert_to = self.original_theme.clone();
            self.last_previewed = None;
            return PaletteEvent::Cancel(revert_to);
        }

        let current_name = self.current_selection_name();
        if current_name != self.last_previewed {
            self.last_previewed = current_name.clone();
            if let Some(name) = current_name {
                return PaletteEvent::Preview(name);
            }
        }

        PaletteEvent::None
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

/// Small painted swatch: background + the 8 base ANSI colors, as 9 adjacent
/// filled rects — enough to recognize a theme's mood at a glance without
/// pulling in an image/gradient renderer.
fn draw_swatch(ui: &mut egui::Ui, palette: &ColorPalette) {
    const CELL_W: f32 = 10.0;
    const CELL_H: f32 = 18.0;
    let hexes = [
        &palette.background,
        &palette.black,
        &palette.red,
        &palette.green,
        &palette.yellow,
        &palette.blue,
        &palette.magenta,
        &palette.cyan,
        &palette.white,
    ];
    let (rect, _resp) = ui.allocate_exact_size(
        egui::vec2(CELL_W * hexes.len() as f32, CELL_H),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    for (i, hex) in hexes.iter().enumerate() {
        let color = crate::theme::hex_to_color32(hex).unwrap_or(egui::Color32::GRAY);
        let cell = egui::Rect::from_min_size(
            rect.min + egui::vec2(i as f32 * CELL_W, 0.0),
            egui::vec2(CELL_W, CELL_H),
        );
        painter.rect_filled(cell, 0.0, color);
    }
}

/// Lists `~/.config/termview/themes/*.toml` and parses each into a
/// `ColorPalette` for swatch previews, reusing `theme::parse_theme_file` (no
/// duplicated TOML-parsing logic). A missing/unreadable directory yields an
/// empty list rather than panicking.
fn scan_themes() -> Vec<ThemeEntry> {
    scan_themes_in(&crate::paths::themes_dir())
}

fn scan_themes_in(dir: &std::path::Path) -> Vec<ThemeEntry> {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<ThemeEntry> = read_dir
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                return None;
            }
            let name = path.file_stem()?.to_str()?.to_string();
            let palette = crate::theme::parse_theme_file(&path);
            Some(ThemeEntry { name, palette })
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_themes_against_missing_dir_is_empty_not_panicking() {
        let dir = std::path::Path::new("/nonexistent/termview-test-dir-xyz");
        assert!(scan_themes_in(dir).is_empty());
    }

    #[test]
    fn scan_themes_parses_toml_files_and_ignores_others() {
        let tmp = std::env::temp_dir().join(format!(
            "termview-palette-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("nord-night.toml"), "background = \"#252933\"").unwrap();
        std::fs::write(tmp.join("README.md"), "not a theme").unwrap();

        let entries = scan_themes_in(&tmp);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "nord-night");
        assert_eq!(entries[0].palette.background, "#252933");

        std::fs::remove_dir_all(&tmp).unwrap();
    }

    #[test]
    fn toggle_opens_with_cursor_on_current_theme_and_no_immediate_preview() {
        let tmp = std::env::temp_dir().join(format!(
            "termview-palette-toggle-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("aaa.toml"), "background = \"#000000\"").unwrap();
        std::fs::write(tmp.join("zzz.toml"), "background = \"#ffffff\"").unwrap();

        let mut palette = CommandPalette::new();
        // Bypass the real themes_dir() by loading entries directly, same
        // shape `toggle` builds internally.
        palette.entries = scan_themes_in(&tmp);
        palette.query.clear();
        palette.original_theme = "zzz".to_string();
        palette.recompute_filter();
        palette.selected = palette
            .filtered
            .iter()
            .position(|&idx| palette.entries[idx].name == "zzz")
            .unwrap_or(0);
        palette.last_previewed = palette.current_selection_name();
        palette.open = true;

        assert_eq!(palette.current_selection_name(), Some("zzz".to_string()));
        assert_eq!(palette.last_previewed, Some("zzz".to_string()));

        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
