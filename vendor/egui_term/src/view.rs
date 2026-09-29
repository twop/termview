use alacritty_terminal::index::Point as TerminalGridPoint;
use alacritty_terminal::term::cell;
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::{Color, NamedColor};
use egui::epaint::RectShape;
use egui::Modifiers;
use egui::MouseWheelUnit;
use egui::Shape;
use egui::Widget;
use egui::{Align2, Painter, Pos2, Rect, Response, Stroke, Vec2};
use egui::{CornerRadius, Key};
use egui::{Id, PointerButton};

use crate::backend::BackendCommand;
use crate::backend::TerminalBackend;
use crate::backend::{LinkAction, MouseButton, SelectionType};
use crate::bindings::Binding;
use crate::bindings::{BindingAction, BindingsLayout, InputKind};
use crate::font::TerminalFont;
use crate::theme::TerminalTheme;
use crate::types::Size;

const EGUI_TERM_WIDGET_ID_PREFIX: &str = "egui_term::instance::";

#[derive(Debug, Clone)]
enum InputAction {
    BackendCall(BackendCommand),
    WriteToClipboard(String),
    Ignore,
}

#[derive(Clone, Default)]
pub struct TerminalViewState {
    is_dragged: bool,
    scroll_pixels: f32,
    current_mouse_position_on_grid: TerminalGridPoint,
}

pub struct TerminalView<'a> {
    widget_id: Id,
    has_focus: bool,
    size: Vec2,
    backend: &'a mut TerminalBackend,
    font: TerminalFont,
    theme: TerminalTheme,
    bindings_layout: BindingsLayout,
}

impl Widget for TerminalView<'_> {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let (layout, painter) =
            ui.allocate_painter(self.size, egui::Sense::click());

        let widget_id = self.widget_id;
        let mut state = ui.memory(|m| {
            m.data
                .get_temp::<TerminalViewState>(widget_id)
                .unwrap_or_default()
        });

        self.focus(&layout)
            .resize(&layout)
            .process_input(&layout, &mut state)
            .show(&mut state, &layout, &painter);

        ui.memory_mut(|m| m.data.insert_temp(widget_id, state));
        layout
    }
}

impl<'a> TerminalView<'a> {
    pub fn new(ui: &mut egui::Ui, backend: &'a mut TerminalBackend) -> Self {
        let widget_id = ui.make_persistent_id(format!(
            "{}{}",
            EGUI_TERM_WIDGET_ID_PREFIX,
            backend.id()
        ));

        Self {
            widget_id,
            has_focus: false,
            size: ui.available_size(),
            backend,
            font: TerminalFont::default(),
            theme: TerminalTheme::default(),
            bindings_layout: BindingsLayout::new(),
        }
    }

    #[inline]
    pub fn set_theme(mut self, theme: TerminalTheme) -> Self {
        self.theme = theme;
        self
    }

    #[inline]
    pub fn set_font(mut self, font: TerminalFont) -> Self {
        self.font = font;
        self
    }

    #[inline]
    pub fn set_focus(mut self, has_focus: bool) -> Self {
        self.has_focus = has_focus;
        self
    }

    #[inline]
    pub fn set_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    #[inline]
    pub fn add_bindings(
        mut self,
        bindings: Vec<(Binding<InputKind>, BindingAction)>,
    ) -> Self {
        self.bindings_layout.add_bindings(bindings);
        self
    }

    fn focus(self, layout: &Response) -> Self {
        if self.has_focus {
            layout.request_focus();
        } else {
            layout.surrender_focus();
        }

        self
    }

    fn resize(self, layout: &Response) -> Self {
        self.backend.process_command(BackendCommand::Resize(
            Size::from(layout.rect.size()),
            self.font.font_measure(&layout.ctx),
        ));

        self
    }

    fn process_input(
        self,
        layout: &Response,
        state: &mut TerminalViewState,
    ) -> Self {
        if !layout.has_focus() || !layout.contains_pointer() {
            return self;
        }

        let modifiers = layout.ctx.input(|i| i.modifiers);
        let events = layout.ctx.input(|i| i.events.clone());
        for event in events {
            let mut input_actions = vec![];

            match event {
                egui::Event::Text(_)
                | egui::Event::Key { .. }
                | egui::Event::Copy
                | egui::Event::Paste(_) => {
                    input_actions.push(process_keyboard_event(
                        event,
                        self.backend,
                        &self.bindings_layout,
                        modifiers,
                    ))
                },
                egui::Event::MouseWheel { unit, delta, .. } => input_actions
                    .push(process_mouse_wheel(
                        state,
                        self.font.font_type().size,
                        unit,
                        delta,
                    )),
                egui::Event::PointerButton {
                    button,
                    pressed,
                    modifiers,
                    pos,
                    ..
                } => input_actions.push(process_button_click(
                    state,
                    layout,
                    self.backend,
                    &self.bindings_layout,
                    button,
                    pos,
                    &modifiers,
                    pressed,
                )),
                egui::Event::PointerMoved(pos) => {
                    input_actions = process_mouse_move(
                        state,
                        layout,
                        self.backend,
                        pos,
                        &modifiers,
                    )
                },
                _ => {},
            };

            for action in input_actions {
                match action {
                    InputAction::BackendCall(cmd) => {
                        self.backend.process_command(cmd);
                    },
                    InputAction::WriteToClipboard(data) => {
                        layout.ctx.copy_text(data);
                    },
                    InputAction::Ignore => {},
                }
            }
        }

        self
    }

    fn show(
        self,
        state: &mut TerminalViewState,
        layout: &Response,
        painter: &Painter,
    ) {
        let content = self.backend.sync();
        let layout_min = layout.rect.min;
        let layout_max = layout.rect.max;
        let cell_height = content.terminal_size.cell_height as f32;
        let cell_width = content.terminal_size.cell_width as f32;
        let global_bg =
            self.theme.get_color(Color::Named(NamedColor::Background));

        let mut shapes = vec![Shape::Rect(RectShape::filled(
            Rect::from_min_max(layout_min, layout_max),
            CornerRadius::ZERO,
            global_bg,
        ))];

        for indexed in content.grid.display_iter() {
            let flags = indexed.cell.flags;
            let is_wide_char_spacer =
                flags.contains(cell::Flags::WIDE_CHAR_SPACER);
            if is_wide_char_spacer {
                continue;
            }

            let is_app_cursor_mode =
                content.terminal_mode.contains(TermMode::APP_CURSOR);
            let is_wide_char = flags.contains(cell::Flags::WIDE_CHAR);
            let is_inverse = flags.contains(cell::Flags::INVERSE);
            let is_dim =
                flags.intersects(cell::Flags::DIM | cell::Flags::DIM_BOLD);
            let is_selected = content
                .selectable_range
                .is_some_and(|r| r.contains(indexed.point));
            let is_hovered_hyperling =
                content.hovered_hyperlink.as_ref().is_some_and(|r| {
                    r.contains(&indexed.point)
                        && r.contains(&state.current_mouse_position_on_grid)
                });

            let x = layout_min.x + (cell_width * indexed.point.column.0 as f32);
            let line_num =
                indexed.point.line.0 + content.grid.display_offset() as i32;
            let y = layout_min.y + (cell_height * line_num as f32);

            let mut fg = self.theme.get_color(indexed.fg);
            let mut bg = self.theme.get_color(indexed.bg);
            let cell_width = if is_wide_char {
                cell_width * 2.0
            } else {
                cell_width
            };

            if is_dim {
                fg = fg.linear_multiply(0.7);
            }

            if is_inverse || is_selected {
                std::mem::swap(&mut fg, &mut bg);
            }

            if global_bg != bg {
                shapes.push(Shape::Rect(RectShape::filled(
                    Rect::from_min_size(
                        Pos2::new(x, y),
                        // + 1.0 is to fill grid border
                        Vec2::new(cell_width + 1., cell_height + 1.),
                    ),
                    CornerRadius::ZERO,
                    bg,
                )));
            }

            // Handle hovered hyperlink underline
            if is_hovered_hyperling {
                let underline_height = y + cell_height;
                shapes.push(Shape::LineSegment {
                    points: [
                        Pos2::new(x, underline_height),
                        Pos2::new(x + cell_width, underline_height),
                    ],
                    stroke: Stroke::new(cell_height * 0.15, fg),
                });
            }

            // Handle cursor rendering
            if content.grid.cursor.point == indexed.point {
                let cursor_color = self.theme.get_color(content.cursor.fg);
                shapes.push(Shape::Rect(RectShape::filled(
                    Rect::from_min_size(
                        Pos2::new(x, y),
                        Vec2::new(cell_width, cell_height),
                    ),
                    CornerRadius::default(),
                    cursor_color,
                )));
            }

            // Draw text content
            if indexed.c != ' ' && indexed.c != '\t' {
                if content.grid.cursor.point == indexed.point
                    && is_app_cursor_mode
                {
                    std::mem::swap(&mut fg, &mut bg);
                }

                shapes.push(painter.fonts_mut(|c| {
                    Shape::text(
                        c,
                        Pos2 {
                            x: x + (cell_width / 2.0),
                            y,
                        },
                        Align2::CENTER_TOP,
                        indexed.c,
                        self.font.font_type(),
                        fg,
                    )
                }));
            }
        }

        painter.extend(shapes);
    }
}

fn process_keyboard_event(
    event: egui::Event,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    modifiers: Modifiers,
) -> InputAction {
    match event {
        egui::Event::Text(text) => {
            process_text_event(&text, modifiers, backend, bindings_layout)
        },
        egui::Event::Paste(text) => InputAction::BackendCall(
            #[cfg(not(any(target_os = "ios", target_os = "macos")))]
            if modifiers.contains(Modifiers::COMMAND | Modifiers::SHIFT) {
                BackendCommand::Write(text.as_bytes().to_vec())
            } else {
                // Hotfix - Send ^V when there's not selection on view.
                BackendCommand::Write([0x16].to_vec())
            },
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            {
                BackendCommand::Write(text.as_bytes().to_vec())
            },
        ),
        egui::Event::Copy => {
            #[cfg(not(any(target_os = "ios", target_os = "macos")))]
            if modifiers.contains(Modifiers::COMMAND | Modifiers::SHIFT) {
                let content = backend.selectable_content();
                InputAction::WriteToClipboard(content)
            } else {
                // Hotfix - Send ^C when there's not selection on view.
                InputAction::BackendCall(BackendCommand::Write([0x3].to_vec()))
            }
            #[cfg(any(target_os = "ios", target_os = "macos"))]
            {
                let content = backend.selectable_content();
                InputAction::WriteToClipboard(content)
            }
        },
        egui::Event::Key {
            key,
            pressed,
            repeat,
            modifiers,
            ..
        } => process_keyboard_key(
            backend,
            bindings_layout,
            key,
            modifiers,
            pressed,
            repeat,
        ),
        _ => InputAction::Ignore,
    }
}

fn process_text_event(
    text: &str,
    modifiers: Modifiers,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
) -> InputAction {
    if let Some(key) = Key::from_name(text) {
        if bindings_layout.get_action(
            InputKind::KeyCode(key),
            modifiers,
            backend.last_content().terminal_mode,
        ) == BindingAction::Ignore
        {
            InputAction::BackendCall(BackendCommand::Write(
                text.as_bytes().to_vec(),
            ))
        } else {
            InputAction::Ignore
        }
    } else {
        InputAction::BackendCall(BackendCommand::Write(
            text.as_bytes().to_vec(),
        ))
    }
}

fn process_keyboard_key(
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    key: Key,
    modifiers: Modifiers,
    pressed: bool,
    repeat: bool,
) -> InputAction {
    let terminal_mode = backend.last_content().terminal_mode;

    // Once the app inside the terminal has negotiated Kitty keyboard mode
    // (`CSI >flags u`), bypass the legacy `bindings_layout` table entirely
    // and encode key events per the Kitty keyboard protocol instead. When
    // no such negotiation ever happened, `terminal_mode` never carries any
    // of these bits and every non-kitty program keeps the exact behavior
    // below, unchanged.
    if terminal_mode.intersects(TermMode::KITTY_KEYBOARD_PROTOCOL) {
        return process_kitty_keyboard_key(
            key,
            modifiers,
            pressed,
            repeat,
            terminal_mode,
        );
    }

    if !pressed {
        return InputAction::Ignore;
    }

    let binding_action = bindings_layout.get_action(
        InputKind::KeyCode(key),
        modifiers,
        terminal_mode,
    );

    match binding_action {
        BindingAction::Char(c) => {
            let mut buf = [0, 0, 0, 0];
            let str = c.encode_utf8(&mut buf);
            InputAction::BackendCall(BackendCommand::Write(
                str.as_bytes().to_vec(),
            ))
        },
        BindingAction::Esc(seq) => InputAction::BackendCall(
            BackendCommand::Write(seq.as_bytes().to_vec()),
        ),
        _ => InputAction::Ignore,
    }
}

/// Encodes a key event using the Kitty keyboard protocol
/// (<https://sw.kovidgoyal.net/kitty/keyboard-protocol/>), used once the
/// program running in the terminal has negotiated one or more of its
/// keyboard modes. Only the mandatory `CSI code;modifiers[:event-type]u`
/// form (and the legacy letter/tilde-suffixed forms for cursor/functional
/// keys, extended with the same modifier/event-type fields) is emitted.
///
/// Not implemented: the `REPORT_ALTERNATE_KEYS`/`REPORT_ASSOCIATED_TEXT`
/// subfields (egui's `Key` doesn't expose the base-layout glyph needed for
/// them), and `REPORT_ALL_KEYS_AS_ESC` (forcing plain, real-modifier-less
/// printable keys through here too would double-send them alongside the
/// `egui::Event::Text` they still generate). Neither is negotiated by the
/// kitty-aware terminal apps this was built for (e.g. Helix only requests
/// disambiguation + event types), so plain printable keys always keep
/// deferring to `process_text_event` here, same as in legacy mode.
fn process_kitty_keyboard_key(
    key: Key,
    modifiers: Modifiers,
    pressed: bool,
    repeat: bool,
    terminal_mode: TermMode,
) -> InputAction {
    let report_event_types = terminal_mode.contains(TermMode::REPORT_EVENT_TYPES);

    if !pressed && !report_event_types {
        return InputAction::Ignore;
    }

    let event_type: Option<u8> = if !pressed {
        Some(3)
    } else if repeat && report_event_types {
        Some(2)
    } else {
        None
    };

    let mod_value = kitty_modifier_value(&modifiers);
    let has_real_modifier = modifiers.ctrl || modifiers.alt || modifiers.mac_cmd;

    if let Some(letter) = kitty_arrow_or_nav_final_byte(key) {
        let seq = if has_real_modifier || modifiers.shift || event_type.is_some() {
            format_legacy_functional(letter, mod_value, event_type)
        } else {
            format!("\x1b[{letter}")
        };
        return InputAction::BackendCall(BackendCommand::Write(seq.into_bytes()));
    }

    if let Some(letter) = kitty_f1_to_f4_final_byte(key) {
        let seq = if has_real_modifier || modifiers.shift || event_type.is_some() {
            format_legacy_functional(letter, mod_value, event_type)
        } else {
            format!("\x1bO{letter}")
        };
        return InputAction::BackendCall(BackendCommand::Write(seq.into_bytes()));
    }

    if let Some(code) = kitty_tilde_functional_code(key) {
        let seq = format_tilde_functional(code, mod_value, event_type);
        return InputAction::BackendCall(BackendCommand::Write(seq.into_bytes()));
    }

    // Shift+Tab is reverse-tab (CBT), not a plain byte - matches the
    // existing legacy binding for the same combo.
    if key == Key::Tab
        && modifiers.shift
        && !has_real_modifier
        && event_type.is_none()
    {
        return InputAction::BackendCall(BackendCommand::Write(
            b"\x1b[Z".to_vec(),
        ));
    }

    if let Some((plain_byte, code)) = kitty_disambiguation_sensitive_key(key) {
        let seq = if has_real_modifier || event_type.is_some() {
            format_csi_u(code, mod_value, event_type)
        } else {
            String::from(plain_byte)
        };
        return InputAction::BackendCall(BackendCommand::Write(seq.into_bytes()));
    }

    // Plain printable keys (letters/digits/punctuation): defer to
    // `process_text_event` unless a real modifier or event-type reporting
    // actually requires an explicit Kitty encoding here.
    if !has_real_modifier && event_type.is_none() {
        return InputAction::Ignore;
    }

    match kitty_printable_code(key) {
        Some(code) => {
            let seq = format_csi_u(code, mod_value, event_type);
            InputAction::BackendCall(BackendCommand::Write(seq.into_bytes()))
        },
        None => InputAction::Ignore,
    }
}

fn kitty_modifier_value(modifiers: &Modifiers) -> u8 {
    let mut bits: u8 = 0;
    if modifiers.shift {
        bits |= 1;
    }
    if modifiers.alt {
        bits |= 2;
    }
    if modifiers.ctrl {
        bits |= 4;
    }
    // Sourced from `mac_cmd` specifically (not `command`, which is also
    // satisfied by plain Ctrl on non-mac platforms - see bindings.rs's
    // pre-existing Cmd/Ctrl conflation in its arrow-key bindings) so the
    // Kitty "super" bit unambiguously means the Cmd/Super/Windows key.
    if modifiers.mac_cmd {
        bits |= 8;
    }
    1 + bits
}

fn format_csi_u(code: u32, mod_value: u8, event_type: Option<u8>) -> String {
    match (mod_value, event_type) {
        (1, None) => format!("\x1b[{code}u"),
        (m, None) => format!("\x1b[{code};{m}u"),
        (m, Some(t)) => format!("\x1b[{code};{m}:{t}u"),
    }
}

fn format_legacy_functional(
    letter: char,
    mod_value: u8,
    event_type: Option<u8>,
) -> String {
    match (mod_value, event_type) {
        (1, None) => format!("\x1b[{letter}"),
        (m, None) => format!("\x1b[1;{m}{letter}"),
        (m, Some(t)) => format!("\x1b[1;{m}:{t}{letter}"),
    }
}

fn format_tilde_functional(
    code: u32,
    mod_value: u8,
    event_type: Option<u8>,
) -> String {
    match (mod_value, event_type) {
        (1, None) => format!("\x1b[{code}~"),
        (m, None) => format!("\x1b[{code};{m}~"),
        (m, Some(t)) => format!("\x1b[{code};{m}:{t}~"),
    }
}

fn kitty_arrow_or_nav_final_byte(key: Key) -> Option<char> {
    match key {
        Key::ArrowUp => Some('A'),
        Key::ArrowDown => Some('B'),
        Key::ArrowRight => Some('C'),
        Key::ArrowLeft => Some('D'),
        Key::End => Some('F'),
        Key::Home => Some('H'),
        _ => None,
    }
}

fn kitty_f1_to_f4_final_byte(key: Key) -> Option<char> {
    match key {
        Key::F1 => Some('P'),
        Key::F2 => Some('Q'),
        Key::F3 => Some('R'),
        Key::F4 => Some('S'),
        _ => None,
    }
}

fn kitty_tilde_functional_code(key: Key) -> Option<u32> {
    match key {
        Key::Insert => Some(2),
        Key::Delete => Some(3),
        Key::PageUp => Some(5),
        Key::PageDown => Some(6),
        Key::F5 => Some(15),
        Key::F6 => Some(17),
        Key::F7 => Some(18),
        Key::F8 => Some(19),
        Key::F9 => Some(20),
        Key::F10 => Some(21),
        Key::F11 => Some(23),
        Key::F12 => Some(24),
        Key::F13 => Some(25),
        Key::F14 => Some(26),
        Key::F15 => Some(28),
        Key::F16 => Some(29),
        Key::F17 => Some(31),
        Key::F18 => Some(32),
        Key::F19 => Some(33),
        Key::F20 => Some(34),
        _ => None,
    }
}

/// Escape/Enter/Tab/Backspace are unambiguous when unmodified (sent as
/// their plain legacy byte), but need the CSI-u form to disambiguate
/// modified presses (e.g. Ctrl+Enter) from other control sequences, or to
/// carry a repeat/release event-type suffix.
fn kitty_disambiguation_sensitive_key(key: Key) -> Option<(char, u32)> {
    match key {
        Key::Escape => Some(('\x1b', 27)),
        Key::Enter => Some(('\r', 13)),
        Key::Tab => Some(('\t', 9)),
        Key::Backspace => Some(('\x7f', 127)),
        _ => None,
    }
}

fn kitty_printable_code(key: Key) -> Option<u32> {
    Some(match key {
        Key::A => 97,
        Key::B => 98,
        Key::C => 99,
        Key::D => 100,
        Key::E => 101,
        Key::F => 102,
        Key::G => 103,
        Key::H => 104,
        Key::I => 105,
        Key::J => 106,
        Key::K => 107,
        Key::L => 108,
        Key::M => 109,
        Key::N => 110,
        Key::O => 111,
        Key::P => 112,
        Key::Q => 113,
        Key::R => 114,
        Key::S => 115,
        Key::T => 116,
        Key::U => 117,
        Key::V => 118,
        Key::W => 119,
        Key::X => 120,
        Key::Y => 121,
        Key::Z => 122,
        Key::Num0 => 48,
        Key::Num1 => 49,
        Key::Num2 => 50,
        Key::Num3 => 51,
        Key::Num4 => 52,
        Key::Num5 => 53,
        Key::Num6 => 54,
        Key::Num7 => 55,
        Key::Num8 => 56,
        Key::Num9 => 57,
        Key::Space => 32,
        Key::Minus => 45,
        Key::Plus => 43,
        Key::Equals => 61,
        Key::Comma => 44,
        Key::Period => 46,
        Key::Semicolon => 59,
        Key::Colon => 58,
        Key::Quote => 39,
        Key::Backtick => 96,
        Key::Slash => 47,
        Key::Backslash => 92,
        Key::Pipe => 124,
        Key::Questionmark => 63,
        Key::Exclamationmark => 33,
        Key::OpenBracket => 91,
        Key::CloseBracket => 93,
        Key::OpenCurlyBracket => 123,
        Key::CloseCurlyBracket => 125,
        _ => return None,
    })
}

fn process_mouse_wheel(
    state: &mut TerminalViewState,
    font_size: f32,
    unit: MouseWheelUnit,
    delta: Vec2,
) -> InputAction {
    match unit {
        MouseWheelUnit::Line => {
            let lines = delta.y.signum() * delta.y.abs().ceil();
            InputAction::BackendCall(BackendCommand::Scroll(lines as i32))
        },
        MouseWheelUnit::Point => {
            state.scroll_pixels -= delta.y;
            let lines = (state.scroll_pixels / font_size).trunc();
            state.scroll_pixels %= font_size;
            if lines != 0.0 {
                InputAction::BackendCall(BackendCommand::Scroll(-lines as i32))
            } else {
                InputAction::Ignore
            }
        },
        MouseWheelUnit::Page => InputAction::Ignore,
    }
}

fn process_button_click(
    state: &mut TerminalViewState,
    layout: &Response,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    button: PointerButton,
    position: Pos2,
    modifiers: &Modifiers,
    pressed: bool,
) -> InputAction {
    match button {
        PointerButton::Primary => process_left_button(
            state,
            layout,
            backend,
            bindings_layout,
            position,
            modifiers,
            pressed,
        ),
        _ => InputAction::Ignore,
    }
}

fn process_left_button(
    state: &mut TerminalViewState,
    layout: &Response,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    position: Pos2,
    modifiers: &Modifiers,
    pressed: bool,
) -> InputAction {
    let terminal_mode = backend.last_content().terminal_mode;
    if terminal_mode.intersects(TermMode::MOUSE_MODE) {
        InputAction::BackendCall(BackendCommand::MouseReport(
            MouseButton::LeftButton,
            *modifiers,
            state.current_mouse_position_on_grid,
            pressed,
        ))
    } else if pressed {
        process_left_button_pressed(state, layout, position)
    } else {
        process_left_button_released(
            state,
            layout,
            backend,
            bindings_layout,
            position,
            modifiers,
        )
    }
}

fn process_left_button_pressed(
    state: &mut TerminalViewState,
    layout: &Response,
    position: Pos2,
) -> InputAction {
    state.is_dragged = true;
    InputAction::BackendCall(build_start_select_command(layout, position))
}

fn process_left_button_released(
    state: &mut TerminalViewState,
    layout: &Response,
    backend: &TerminalBackend,
    bindings_layout: &BindingsLayout,
    position: Pos2,
    modifiers: &Modifiers,
) -> InputAction {
    state.is_dragged = false;
    if layout.double_clicked() || layout.triple_clicked() {
        InputAction::BackendCall(build_start_select_command(layout, position))
    } else {
        let terminal_content = backend.last_content();
        let binding_action = bindings_layout.get_action(
            InputKind::Mouse(PointerButton::Primary),
            *modifiers,
            terminal_content.terminal_mode,
        );

        if binding_action == BindingAction::LinkOpen {
            InputAction::BackendCall(BackendCommand::ProcessLink(
                LinkAction::Open,
                state.current_mouse_position_on_grid,
            ))
        } else {
            InputAction::Ignore
        }
    }
}

fn build_start_select_command(
    layout: &Response,
    cursor_position: Pos2,
) -> BackendCommand {
    let selection_type = if layout.double_clicked() {
        SelectionType::Semantic
    } else if layout.triple_clicked() {
        SelectionType::Lines
    } else {
        SelectionType::Simple
    };

    BackendCommand::SelectStart(
        selection_type,
        cursor_position.x - layout.rect.min.x,
        cursor_position.y - layout.rect.min.y,
    )
}

fn process_mouse_move(
    state: &mut TerminalViewState,
    layout: &Response,
    backend: &TerminalBackend,
    position: Pos2,
    modifiers: &Modifiers,
) -> Vec<InputAction> {
    let terminal_content = backend.last_content();
    let cursor_x = position.x - layout.rect.min.x;
    let cursor_y = position.y - layout.rect.min.y;
    state.current_mouse_position_on_grid = TerminalBackend::selection_point(
        cursor_x,
        cursor_y,
        &terminal_content.terminal_size,
        terminal_content.grid.display_offset(),
    );

    let mut actions = vec![];
    // Handle command or selection update based on terminal mode and modifiers
    if state.is_dragged {
        let terminal_mode = terminal_content.terminal_mode;
        let cmd = if terminal_mode.contains(TermMode::MOUSE_MOTION)
            && modifiers.is_none()
        {
            InputAction::BackendCall(BackendCommand::MouseReport(
                MouseButton::LeftMove,
                *modifiers,
                state.current_mouse_position_on_grid,
                true,
            ))
        } else {
            InputAction::BackendCall(BackendCommand::SelectUpdate(
                cursor_x, cursor_y,
            ))
        };

        actions.push(cmd);
    }

    // Handle link hover if applicable
    if modifiers.command_only() {
        actions.push(InputAction::BackendCall(BackendCommand::ProcessLink(
            LinkAction::Hover,
            state.current_mouse_position_on_grid,
        )));
    }

    actions
}

#[cfg(test)]
mod kitty_keyboard_tests {
    use super::*;

    fn assert_write(action: InputAction, expected: &[u8]) {
        match action {
            InputAction::BackendCall(BackendCommand::Write(bytes)) => {
                assert_eq!(bytes, expected);
            },
            other => panic!("expected Write({expected:?}), got {other:?}"),
        }
    }

    fn assert_ignored(action: InputAction) {
        assert!(
            matches!(action, InputAction::Ignore),
            "expected Ignore, got {action:?}"
        );
    }

    #[test]
    fn cmd_s_is_encoded_with_super_modifier() {
        // This is the originally-reported bug: Cmd+S must now reach the
        // PTY (as a Kitty CSI-u sequence) instead of being silently
        // dropped by the legacy bindings table.
        let action = process_kitty_keyboard_key(
            Key::S,
            Modifiers::MAC_CMD,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_write(action, b"\x1b[115;9u");
    }

    #[test]
    fn ctrl_a_is_encoded_via_csi_u() {
        let action = process_kitty_keyboard_key(
            Key::A,
            Modifiers::CTRL,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_write(action, b"\x1b[97;5u");
    }

    #[test]
    fn plain_letter_defers_to_text_event() {
        let action = process_kitty_keyboard_key(
            Key::A,
            Modifiers::NONE,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_ignored(action);
    }

    #[test]
    fn shift_letter_alone_defers_to_text_event() {
        let action = process_kitty_keyboard_key(
            Key::A,
            Modifiers::SHIFT,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_ignored(action);
    }

    #[test]
    fn shift_tab_is_reverse_tab() {
        let action = process_kitty_keyboard_key(
            Key::Tab,
            Modifiers::SHIFT,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_write(action, b"\x1b[Z");
    }

    #[test]
    fn bare_arrow_up_matches_legacy_bytes() {
        let action = process_kitty_keyboard_key(
            Key::ArrowUp,
            Modifiers::NONE,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_write(action, b"\x1b[A");
    }

    #[test]
    fn ctrl_arrow_up_matches_legacy_shape() {
        let action = process_kitty_keyboard_key(
            Key::ArrowUp,
            Modifiers::CTRL,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_write(action, b"\x1b[1;5A");
    }

    #[test]
    fn release_is_ignored_without_report_event_types() {
        let action = process_kitty_keyboard_key(
            Key::A,
            Modifiers::CTRL,
            false,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_ignored(action);
    }

    #[test]
    fn release_is_encoded_with_event_type_suffix_when_enabled() {
        let mode =
            TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES;
        let action =
            process_kitty_keyboard_key(Key::A, Modifiers::CTRL, false, false, mode);
        assert_write(action, b"\x1b[97;5:3u");
    }

    #[test]
    fn repeat_is_encoded_with_event_type_suffix_when_enabled() {
        let mode =
            TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES;
        let action =
            process_kitty_keyboard_key(Key::A, Modifiers::CTRL, true, true, mode);
        assert_write(action, b"\x1b[97;5:2u");
    }

    #[test]
    fn plain_release_with_report_event_types_is_reported() {
        // Even a plain, unmodified letter's release must be reported once
        // REPORT_EVENT_TYPES is active - it can't be deferred to a Text
        // event (releases never generate one).
        let mode =
            TermMode::DISAMBIGUATE_ESC_CODES | TermMode::REPORT_EVENT_TYPES;
        let action =
            process_kitty_keyboard_key(Key::A, Modifiers::NONE, false, false, mode);
        assert_write(action, b"\x1b[97;1:3u");
    }

    #[test]
    fn bare_enter_is_plain_byte() {
        let action = process_kitty_keyboard_key(
            Key::Enter,
            Modifiers::NONE,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_write(action, b"\r");
    }

    #[test]
    fn alt_backspace_is_encoded_via_csi_u() {
        let action = process_kitty_keyboard_key(
            Key::Backspace,
            Modifiers::ALT,
            true,
            false,
            TermMode::DISAMBIGUATE_ESC_CODES,
        );
        assert_write(action, b"\x1b[127;3u");
    }
}
