// Verifies the alacritty_terminal-side wiring this fork relies on for
// Kitty keyboard protocol support: with `Config::kitty_keyboard: true`
// (the change made in `src/backend/mod.rs`), an app's `CSI >flags u` /
// `CSI ?u` escape sequences must actually reach `Term`'s keyboard-mode
// handlers and produce a real `PtyWrite` reply - this is the "query
// round-trip" the implementation plan's grounding relied on, exercised
// here directly against `alacritty_terminal` rather than through a full
// GUI session (which this test suite can't drive headlessly).

use std::cell::RefCell;
use std::rc::Rc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};

#[derive(Clone, Default)]
struct RecordingListener(Rc<RefCell<Vec<String>>>);

impl EventListener for RecordingListener {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            self.0.borrow_mut().push(text);
        }
    }
}

fn new_term() -> (Term<RecordingListener>, RecordingListener) {
    let listener = RecordingListener::default();
    let config = Config {
        kitty_keyboard: true,
        ..Config::default()
    };
    let size = TermSize::new(80, 24);
    let term = Term::new(config, &size, listener.clone());
    (term, listener)
}

#[test]
fn query_before_any_mode_push_reports_zero() {
    let (mut term, listener) = new_term();
    let mut parser = Processor::<StdSyncHandler>::new();

    parser.advance(&mut term, b"\x1b[?u");

    let replies = listener.0.borrow();
    assert_eq!(replies.as_slice(), [String::from("\x1b[?0u")]);
}

#[test]
fn pushing_disambiguate_mode_is_reflected_in_a_later_query() {
    let (mut term, listener) = new_term();
    let mut parser = Processor::<StdSyncHandler>::new();

    // Push "disambiguate escape codes" (flag 1).
    parser.advance(&mut term, b"\x1b[>1u");
    parser.advance(&mut term, b"\x1b[?u");

    let replies = listener.0.borrow();
    assert_eq!(replies.as_slice(), [String::from("\x1b[?1u")]);
}

#[test]
fn pop_restores_previous_mode() {
    let (mut term, listener) = new_term();
    let mut parser = Processor::<StdSyncHandler>::new();

    parser.advance(&mut term, b"\x1b[>1u"); // push disambiguate
    parser.advance(&mut term, b"\x1b[>3u"); // push disambiguate + report event types
    parser.advance(&mut term, b"\x1b[<u"); // pop back to the first push
    parser.advance(&mut term, b"\x1b[?u");

    let replies = listener.0.borrow();
    assert_eq!(replies.as_slice(), [String::from("\x1b[?1u")]);
}

#[test]
fn kitty_keyboard_protocol_disabled_by_default_ignores_queries() {
    // Sanity check on the *unpatched* default: without the config flag
    // this fork sets, queries get no reply at all (the pre-existing
    // upstream behavior we changed).
    let listener = RecordingListener::default();
    let size = TermSize::new(80, 24);
    let mut term = Term::new(Config::default(), &size, listener.clone());
    let mut parser = Processor::<StdSyncHandler>::new();

    parser.advance(&mut term, b"\x1b[?u");

    assert!(listener.0.borrow().is_empty());
}
