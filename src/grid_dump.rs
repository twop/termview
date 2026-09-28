// egui_term exposes only the on-screen grid (no scrollback/raw stdout), so this is a
// best-effort snapshot of whatever was visible right before the inner program exited.
pub fn dump_screen_text(terminal_backend: &mut egui_term::TerminalBackend) -> String {
    let content = terminal_backend.sync();

    let mut rows: Vec<String> = Vec::new();
    let mut row = String::new();
    let mut current_line = None;
    for indexed in content.grid.display_iter() {
        if current_line.is_some() && current_line != Some(indexed.point.line) {
            rows.push(std::mem::take(&mut row));
        }
        current_line = Some(indexed.point.line);
        row.push(indexed.c);
    }
    rows.push(row);

    rows.iter()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}
