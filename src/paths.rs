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

pub fn config_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::PathBuf::from(home).join(".config/termview")
}

pub fn config_file_path() -> std::path::PathBuf {
    config_dir().join("config.toml")
}

pub fn themes_dir() -> std::path::PathBuf {
    config_dir().join("themes")
}

pub fn theme_file_path(name: &str) -> std::path::PathBuf {
    themes_dir().join(format!("{name}.toml"))
}
