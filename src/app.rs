use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use crate::config::{Config, KeyCombo};
use crate::ipc::{DaemonRequest, DaemonResponse};
use crate::palette::CommandPalette;
use crate::session::Session;
use crate::theme::LoadedTheme;
use crate::tray::{Tray, TrayAction};

const FONT_SIZE: f32 = 12.0;

// egui needs one render cycle after a window becomes visible before a
// Focus command reliably takes effect — see `pending_focus_frames`.
const FOCUS_DELAY_FRAMES: u32 = 2;

// Matches the 220ms ease-out-cubic timing already established elsewhere in
// this codebase family (script_app.rs's widget animations, and the
// Hammerspoon leader-menu's own SPLIT_TRANSITION_DURATION).
const SHOW_ANIM_DURATION: Duration = Duration::from_millis(220);

pub type ReplyTx = Sender<DaemonResponse>;

pub struct TermViewApp {
    sessions: HashMap<String, Session>,
    active_workspace: Option<String>,
    next_backend_id: u64,
    pty_tx: Sender<(u64, egui_term::PtyEvent)>,
    pty_rx: Receiver<(u64, egui_term::PtyEvent)>,
    ipc_rx: Receiver<(DaemonRequest, ReplyTx)>,
    tray: Tray,
    tray_rx: Receiver<TrayAction>,
    config: Config,
    palette_shortcut: KeyCombo,
    theme: LoadedTheme,
    command_palette: CommandPalette,
    close_window_pressed: bool,
    toggle_palette_pressed: bool,
    /// Frames remaining before a Focus command is sent, or `None` when idle.
    pending_focus_frames: Option<u32>,
    /// When the window was last shown, driving the fade/scale/slide-in. `None`
    /// once the animation has finished (or none is in progress).
    show_animation: Option<Instant>,
    /// Whether we last told the window to be visible — used to skip
    /// `begin_show`'s animation/focus dance when the window is already on
    /// screen (e.g. switching workspaces, or re-triggering an already-open
    /// one) so it doesn't visibly flash back to its hidden state and re-animate.
    currently_shown: bool,
    /// The viewport's OS focus state as of the previous frame — closing on
    /// focus loss is edge-triggered (true→false only), not level-triggered,
    /// the same way shelv's own auto-hide-on-blur works.
    prev_focused: bool,
    /// Whether it's safe to act on `prev_focused`/`is_focused` yet. Right
    /// after showing, `focused` can read a spurious `false` for a frame or
    /// two before the OS catches up (shelv has no equivalent guard, but it
    /// doesn't need one since it doesn't defer its own Focus command) —
    /// without this, the very first spurious dip immediately closed the
    /// window right after it opened. Armed only once real focus is observed
    /// after the deliberate post-show settling window has passed.
    blur_armed: bool,
}

impl TermViewApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        listener: std::os::unix::net::UnixListener,
    ) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        crate::fonts::register_fonts(&cc.egui_ctx);

        let (pty_tx, pty_rx) = std::sync::mpsc::channel();
        let (tray_tx, tray_rx) = std::sync::mpsc::channel();
        let tray = Tray::new(tray_tx, cc.egui_ctx.clone());

        let (ipc_tx, ipc_rx) = std::sync::mpsc::channel();
        crate::daemon::spawn_accept_loop(listener, ipc_tx, cc.egui_ctx.clone());

        let config = Config::load();
        let palette_shortcut = config.parsed_palette_shortcut();
        let theme = LoadedTheme::load(&config.theme);

        Self {
            sessions: HashMap::new(),
            active_workspace: None,
            next_backend_id: 0,
            pty_tx,
            pty_rx,
            ipc_rx,
            tray,
            tray_rx,
            config,
            palette_shortcut,
            theme,
            command_palette: CommandPalette::new(),
            close_window_pressed: false,
            toggle_palette_pressed: false,
            pending_focus_frames: None,
            show_animation: None,
            currently_shown: false,
            prev_focused: false,
            blur_armed: false,
        }
    }

    /// Shows the window, kicking off the fade/scale/slide-in animation and
    /// deferring the actual Focus command by a couple of frames (egui/winit
    /// only honor Focus reliably once the window has rendered at least once
    /// since becoming visible).
    ///
    /// No-ops if the window is already shown: e.g. switching to a different
    /// workspace, or re-triggering an already-open one, shouldn't replay the
    /// reveal animation on top of content that never actually left the screen.
    fn begin_show(&mut self, ctx: &egui::Context) {
        if self.currently_shown {
            return;
        }
        self.currently_shown = true;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.pending_focus_frames = Some(FOCUS_DELAY_FRAMES);
        self.show_animation = Some(Instant::now());
        ctx.request_repaint();
    }

    /// Hides the window and returns keyboard focus to whatever was frontmost
    /// before termview was summoned. `ViewportCommand::Visible(false)` alone
    /// does not do the latter (confirmed empirically — focus stays stuck on
    /// termview with nothing visible), so this goes straight to AppKit; see
    /// `crate::macos::hide_app`.
    fn hide_window(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "macos")]
        crate::macos::hide_app();
        #[cfg(not(target_os = "macos"))]
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        self.currently_shown = false;
        self.blur_armed = false;
        ctx.request_repaint();
    }

    fn sync_tray(&mut self) {
        let mut persistent_workspaces: Vec<String> = self
            .sessions
            .values()
            .filter(|session| session.persistent)
            .map(|session| session.workspace.clone())
            .collect();
        persistent_workspaces.sort();
        self.tray.rebuild(&persistent_workspaces);
    }

    #[allow(clippy::too_many_arguments)]
    fn open_workspace(
        &mut self,
        ctx: &egui::Context,
        workspace: String,
        command_string: String,
        report: Option<String>,
        persistent: bool,
        width: Option<u32>,
        height: Option<u32>,
        cwd: Option<String>,
    ) -> DaemonResponse {
        let parts = match shell_words::split(&command_string) {
            Ok(parts) if !parts.is_empty() => parts,
            Ok(_) => return DaemonResponse::Err("empty command".to_string()),
            Err(err) => {
                return DaemonResponse::Err(format!("failed to parse command: {err}"));
            }
        };
        let program = parts[0].clone();
        let args = parts[1..].to_vec();

        let needs_restart = self
            .sessions
            .get(&workspace)
            .is_some_and(|existing| existing.command_string != command_string);
        let is_new = !self.sessions.contains_key(&workspace) || needs_restart;

        if needs_restart {
            // Dropping the old backend sends Msg::Shutdown to end its PTY.
            self.sessions.remove(&workspace);
        }

        if is_new {
            let id = self.next_backend_id;
            self.next_backend_id += 1;

            let backend = match egui_term::TerminalBackend::new(
                id,
                ctx.clone(),
                self.pty_tx.clone(),
                egui_term::BackendSettings {
                    shell: program,
                    args,
                    working_directory: cwd.as_deref().map(crate::paths::expand_tilde),
                },
            ) {
                Ok(backend) => backend,
                Err(err) => return DaemonResponse::Err(format!("failed to spawn: {err}")),
            };

            self.sessions.insert(
                workspace.clone(),
                Session {
                    workspace: workspace.clone(),
                    command_string,
                    persistent,
                    report,
                    backend_id: id,
                    backend,
                },
            );
        } else if let Some(existing) = self.sessions.get_mut(&workspace) {
            // Reusing the running session: a later `open` may retag policy
            // (persistent/report) without restarting the PTY.
            existing.persistent = persistent;
            existing.report = report;
        }

        self.active_workspace = Some(workspace);

        self.begin_show(ctx);
        if let (Some(w), Some(h)) = (width, height) {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::Vec2::new(
                w as f32, h as f32,
            )));
        }

        self.sync_tray();

        DaemonResponse::Ok
    }

    fn handle_request(&mut self, ctx: &egui::Context, request: DaemonRequest, reply_tx: ReplyTx) {
        match request {
            DaemonRequest::Open {
                workspace,
                command_string,
                report,
                persistent,
                width,
                height,
                cwd,
            } => {
                let response = self.open_workspace(
                    ctx,
                    workspace,
                    command_string,
                    report,
                    persistent,
                    width,
                    height,
                    cwd,
                );
                let _ = reply_tx.send(response);
            }
            DaemonRequest::Quit => {
                let _ = reply_tx.send(DaemonResponse::Ok);
                self.sessions.clear();
                let _ = std::fs::remove_file(crate::ipc::socket_path());
                // Give the connection thread a moment to flush the reply before
                // the whole process (all threads) disappears.
                std::thread::sleep(std::time::Duration::from_millis(100));
                std::process::exit(0);
            }
        }
    }

    fn handle_tray_action(&mut self, ctx: &egui::Context, action: TrayAction) {
        match action {
            TrayAction::Show(workspace) => {
                if self.sessions.contains_key(&workspace) {
                    self.active_workspace = Some(workspace);
                    self.begin_show(ctx);
                }
            }
            TrayAction::Close(workspace) => {
                self.sessions.remove(&workspace);
                if self.active_workspace.as_deref() == Some(workspace.as_str()) {
                    self.active_workspace = None;
                    self.hide_window(ctx);
                }
                self.sync_tray();
                ctx.request_repaint();
            }
            TrayAction::Quit => {
                self.sessions.clear();
                let _ = std::fs::remove_file(crate::ipc::socket_path());
                std::process::exit(0);
            }
        }
    }

    fn handle_pty_exit(&mut self, ctx: &egui::Context, backend_id: u64) {
        let workspace = self
            .sessions
            .iter()
            .find(|(_, session)| session.backend_id == backend_id)
            .map(|(workspace, _)| workspace.clone());

        let Some(workspace) = workspace else {
            return;
        };

        if let Some(mut session) = self.sessions.remove(&workspace) {
            let screen_text = crate::grid_dump::dump_screen_text(&mut session.backend);
            if let Some(template) = &session.report {
                crate::report::fire(template, &screen_text);
            }

            if self.active_workspace.as_deref() == Some(workspace.as_str()) {
                self.active_workspace = None;
                self.hide_window(ctx);
            }

            if session.persistent {
                self.sync_tray();
            }
        }

        ctx.request_repaint();
    }

    fn handle_close_window(&mut self, ctx: &egui::Context) {
        let Some(workspace) = self.active_workspace.clone() else {
            return;
        };
        let Some(session) = self.sessions.get(&workspace) else {
            return;
        };

        if !session.persistent {
            // Ephemeral: Cmd+W is a manual abort, not completion — no report fires.
            self.sessions.remove(&workspace);
            self.active_workspace = None;
        }

        self.hide_window(ctx);
    }

    fn apply_palette_event(&mut self, event: crate::palette::PaletteEvent) {
        use crate::palette::PaletteEvent;
        match event {
            PaletteEvent::None => {}
            PaletteEvent::Preview(name) => {
                self.theme = LoadedTheme::load(&name);
            }
            PaletteEvent::Commit(name) => {
                self.config.theme = name.clone();
                if let Err(err) = self.config.save() {
                    eprintln!("termview: failed to save config: {err}");
                }
                self.theme = LoadedTheme::load(&name);
            }
            PaletteEvent::Cancel(name) => {
                self.theme = LoadedTheme::load(&name);
            }
        }
    }
}

impl eframe::App for TermViewApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        let mut close_triggered = false;
        let mut palette_triggered = false;
        raw_input.events.retain(|event| {
            if let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            {
                if *key == egui::Key::W && modifiers.mac_cmd {
                    close_triggered = true;
                    return false;
                }
                if self.palette_shortcut.matches(*key, modifiers) {
                    palette_triggered = true;
                    return false;
                }
            }
            true
        });
        if close_triggered {
            self.close_window_pressed = true;
        }
        if palette_triggered {
            self.toggle_palette_pressed = true;
        }

        // egui_term's TerminalView only forwards keyboard events when the
        // pointer is *also* hovering it (has_focus() alone isn't enough — see
        // egui_term::view::process_input), so `.set_focus(true)` and even a
        // real OS window focus aren't sufficient by themselves: right after
        // showing, the real cursor is usually still wherever it was before,
        // nowhere near the (centered) window, and typed keys are silently
        // dropped until the user manually moves the mouse over it. Inject a
        // synthetic pointer position at the window's center for the same
        // frames the Focus command is deferred over — this only updates
        // egui's own hover tracking, the real OS cursor never moves, and it
        // sticks (no further correction needed) until the real cursor
        // actually reports a different position.
        if self.pending_focus_frames.is_some() {
            if let Some(rect) = raw_input.screen_rect {
                raw_input.events.push(egui::Event::PointerMoved(rect.center()));
            }
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok((request, reply_tx)) = self.ipc_rx.try_recv() {
            self.handle_request(ctx, request, reply_tx);
        }

        while let Ok(action) = self.tray_rx.try_recv() {
            self.handle_tray_action(ctx, action);
        }

        while let Ok((id, event)) = self.pty_rx.try_recv() {
            if matches!(event, egui_term::PtyEvent::Exit) {
                self.handle_pty_exit(ctx, id);
            }
        }

        if self.close_window_pressed {
            self.close_window_pressed = false;
            self.handle_close_window(ctx);
        }

        if self.toggle_palette_pressed {
            self.toggle_palette_pressed = false;
            let event = self.command_palette.toggle(&self.config.theme);
            self.apply_palette_event(event);
        }

        if let Some(remaining) = self.pending_focus_frames {
            if remaining == 0 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                self.pending_focus_frames = None;
            } else {
                self.pending_focus_frames = Some(remaining - 1);
                ctx.request_repaint();
            }
        }

        if let Some(started_at) = self.show_animation {
            if started_at.elapsed() >= SHOW_ANIM_DURATION {
                self.show_animation = None;
            } else {
                ctx.request_repaint();
            }
        }

        // Close (same as Cmd+W: kill if ephemeral, just hide if persistent)
        // when the window loses OS focus — edge-triggered on true→false only,
        // so this fires once per click-away, not every frame spent unfocused.
        if self.currently_shown {
            let is_focused = ctx.input(|i| i.viewport().focused.unwrap_or(false));
            if !self.blur_armed {
                // Don't trust any reading until real focus has actually
                // landed after the deliberate post-show settling window —
                // right after showing, `focused` can read a spurious `false`
                // for a frame or two while the OS is still catching up, which
                // would otherwise look identical to a genuine click-away.
                if self.pending_focus_frames.is_none() && is_focused {
                    self.blur_armed = true;
                    self.prev_focused = true;
                }
            } else if self.prev_focused && !is_focused {
                self.handle_close_window(ctx);
            }
            if self.blur_armed {
                self.prev_focused = is_focused;
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Fade + scale + slide-in over `SHOW_ANIM_DURATION`, purely as an egui
        // layer transform/opacity — the OS window itself never moves or resizes.
        let animation_progress = self.show_animation.map(|started_at| {
            let t = started_at.elapsed().as_secs_f32() / SHOW_ANIM_DURATION.as_secs_f32();
            ease_out_cubic(t.clamp(0.0, 1.0))
        });

        // `ui.set_opacity()` is per-Painter state and does *not* propagate into
        // `CentralPanel::show`'s own freshly-created inner Ui (confirmed
        // empirically — it had no visible effect), unlike the layer-wide
        // `transform_layer_shapes` used below for scale/slide. So opacity is
        // applied the same way: post-hoc, by walking every shape this frame
        // added to the layer and multiplying its color's alpha directly.
        let full_rect = ui.max_rect();
        let layer_id = ui.layer_id();
        let ctx = ui.ctx().clone();
        let shapes_start = ctx.graphics_mut(|g| g.entry(layer_id).next_idx());

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .inner_margin(self.config.padding)
                    .corner_radius(self.config.corner_radius)
                    .fill(self.theme.semantic.background)
                    .stroke(egui::Stroke::new(1.0, self.theme.semantic.border)),
            )
            .show(ui, |ui| {
                let Some(workspace) = self.active_workspace.clone() else {
                    return;
                };
                let Some(session) = self.sessions.get_mut(&workspace) else {
                    return;
                };

                let terminal_font = egui_term::TerminalFont::new(egui_term::FontSettings {
                    font_type: egui::FontId::monospace(FONT_SIZE),
                });

                // Keep the terminal unfocused while the command palette is
                // open: egui_term forwards keystrokes to the shell whenever
                // the pointer merely hovers it (see the pointer-injection
                // comment in raw_input_hook above), so leaving it focused
                // here would leak typed characters into the shell
                // underneath the palette's own search box.
                let terminal = egui_term::TerminalView::new(ui, &mut session.backend)
                    .set_focus(!self.command_palette.is_open())
                    .set_font(terminal_font)
                    .set_theme(self.theme.terminal_theme())
                    .set_size(ui.available_size());

                ui.add(terminal);
            });

        let palette_event = self.command_palette.show(ui.ctx(), &self.theme.palette);
        self.apply_palette_event(palette_event);

        if let Some(progress) = animation_progress {
            let shapes_end = ctx.graphics_mut(|g| g.entry(layer_id).next_idx());
            ctx.graphics_mut(|graphics| {
                let list = graphics.entry(layer_id);
                for idx in shapes_start.0..shapes_end.0 {
                    list.mutate_shape(egui::layers::ShapeIdx(idx), |clipped| {
                        epaint::shape_transform::adjust_colors(&mut clipped.shape, move |color| {
                            if *color != egui::Color32::PLACEHOLDER {
                                *color = color.gamma_multiply(progress);
                            }
                        });
                    });
                }
            });

            let scale = lerp(0.92, 1.0, progress);
            let slide = lerp(24.0, 0.0, progress);
            let pivot = full_rect.center().to_vec2();
            let translation = pivot * (1.0 - scale) + egui::Vec2::new(0.0, slide);
            let transform = emath::TSTransform::new(translation, scale);
            ctx.transform_layer_shapes(layer_id, transform);
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}
