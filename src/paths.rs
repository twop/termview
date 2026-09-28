// working_directory goes straight to the OS (no shell in between), so "~" needs
// expanding ourselves — the shell isn't there to do it for us.
pub fn expand_tilde(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home).join(rest);
        }
    } else if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return std::path::PathBuf::from(home);
        }
    }
    std::path::PathBuf::from(path)
}
