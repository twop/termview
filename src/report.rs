use std::process::Command;

/// Substitutes `{stdout}` in `template` with the URL-encoded `screen_text`, then
/// shells out to macOS `open` so whatever URL scheme is registered (e.g. a
/// Hammerspoon `hammerspoon://` handler) picks up the result.
pub fn fire(template: &str, screen_text: &str) {
    let encoded = urlencoding::encode(screen_text);
    let url = template.replace("{stdout}", &encoded);

    if let Err(err) = Command::new("open").arg(&url).spawn() {
        eprintln!("termview: failed to invoke report callback: {err}");
    }
}
