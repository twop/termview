// eframe/winit have no portable "hide this window but don't quit" concept,
// and `ViewportCommand::Visible(false)` alone doesn't yield the app's
// activation back to whatever was frontmost before termview was summoned —
// confirmed empirically: the window disappears but keyboard focus stays
// stuck on termview instead of returning to the previous app. Going
// straight to AppKit's `NSApplication hide:` fixes both at once (hiding the
// whole (single-window) app and returning focus) — the same approach
// shelv's own tray-hide behavior uses.
#[cfg(target_os = "macos")]
pub fn hide_app() {
    use objc2::rc::Id;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, msg_send_id};
    unsafe {
        let app: Id<AnyObject> = msg_send_id![class!(NSApplication), sharedApplication];
        let arg = app.as_ref();
        let _: () = msg_send![&app, hide: arg];
    }
}
