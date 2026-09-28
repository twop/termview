use std::io::ErrorKind;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::mpsc::Sender;

use crate::app::{ReplyTx, TermViewApp};
use crate::ipc::{self, DaemonRequest, DaemonResponse};

pub fn run() -> eframe::Result {
    let listener = bind_listener();

    let native_options = eframe::NativeOptions {
        // Deliberately no `.with_visible(false)` here: creating the viewport
        // already hidden pins the daemon at ~100% CPU (a present/redraw retry
        // loop against a window that has never been shown, confirmed empirically).
        // Toggling visibility later via `ViewportCommand::Visible` after the
        // window has legitimately shown at least once is fine — that's how
        // sessions get hidden on Cmd+W / shown again.
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 900.0])
            .with_min_inner_size([300.0, 220.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_has_shadow(false)
            .with_resizable(true)
            .with_always_on_top(),
        event_loop_builder: Some(Box::new(|builder| {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
                builder.with_activation_policy(ActivationPolicy::Accessory);
            }
        })),
        // Default resting size/position is centered 800x400 on every fresh
        // daemon launch — not "remembered from last time": eframe's
        // persist_window would otherwise silently override this default with
        // whatever geometry a previous run (or an old build, during
        // development) left on disk. --width/--height on `open` still apply
        // via ViewportCommand for that invocation, and a manual resize is
        // naturally kept for as long as this daemon process stays alive
        // (hide/show reuses the same window) — it just doesn't survive quit.
        centered: true,
        persist_window: false,
        run_and_return: true,
        ..Default::default()
    };

    eframe::run_native(
        "termview",
        native_options,
        Box::new(move |cc| Ok(Box::new(TermViewApp::new(cc, listener)))),
    )
}

/// Binds the daemon's Unix socket, recovering from a stale file left behind by
/// a crashed previous daemon. If another daemon is genuinely still alive and
/// listening, this instance has lost the startup race and exits immediately.
fn bind_listener() -> UnixListener {
    let path = ipc::socket_path();
    match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(err) if err.kind() == ErrorKind::AddrInUse => {
            if UnixStream::connect(&path).is_ok() {
                eprintln!("termview: a daemon is already running, exiting");
                std::process::exit(1);
            }
            let _ = std::fs::remove_file(&path);
            UnixListener::bind(&path)
                .expect("failed to bind daemon socket after removing stale file")
        }
        Err(err) => panic!("failed to bind daemon socket: {err}"),
    }
}

/// Spawns the accept loop on a background thread. Must be called with a real
/// `egui::Context` (available only once the eframe app/window exists) because
/// each accepted connection needs to call `ctx.request_repaint()` after
/// enqueueing its request — otherwise, while the window is hidden and idle,
/// eframe's event loop never wakes up to drain it.
pub fn spawn_accept_loop(
    listener: UnixListener,
    ipc_tx: Sender<(DaemonRequest, ReplyTx)>,
    ctx: egui::Context,
) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let ipc_tx = ipc_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || handle_connection(stream, ipc_tx, ctx));
        }
    });
}

fn handle_connection(
    stream: UnixStream,
    ipc_tx: Sender<(DaemonRequest, ReplyTx)>,
    ctx: egui::Context,
) {
    let read_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = ipc::buf_reader(read_stream);
    let request: DaemonRequest = match ipc::read_message(&mut reader) {
        Ok(request) => request,
        Err(_) => return,
    };

    let (reply_tx, reply_rx) = std::sync::mpsc::channel::<DaemonResponse>();
    if ipc_tx.send((request, reply_tx)).is_err() {
        return;
    }
    ctx.request_repaint();

    if let Ok(response) = reply_rx.recv() {
        let mut stream = stream;
        let _ = ipc::write_message(&mut stream, &response);
    }
}
