pub struct Session {
    pub workspace: String,
    pub command_string: String,
    pub persistent: bool,
    pub report: Option<String>,
    pub backend_id: u64,
    pub backend: egui_term::TerminalBackend,
}
