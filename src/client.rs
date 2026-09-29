use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use crate::cli::{Command as CliCommand, OpenArgs};
use crate::ipc::{self, DaemonRequest, DaemonResponse};

pub fn run(command: CliCommand) -> ExitCode {
    match command {
        CliCommand::Open(args) => run_open(args),
        CliCommand::Quit => run_quit(),
        CliCommand::Daemon => unreachable!("__daemon is dispatched to daemon::run in main"),
    }
}

fn run_open(args: OpenArgs) -> ExitCode {
    let request = DaemonRequest::Open {
        workspace: args.workspace,
        command_string: args.command_string,
        shell: args.shell,
        report: args.report,
        persistent: args.persistent,
        width: args.width,
        height: args.height,
        cwd: args.cwd,
    };
    send_request(request)
}

fn run_quit() -> ExitCode {
    match UnixStream::connect(ipc::socket_path()) {
        Ok(stream) => reply_and_exit(stream, &DaemonRequest::Quit),
        Err(_) => {
            println!("termview: no daemon running");
            ExitCode::SUCCESS
        }
    }
}

fn send_request(request: DaemonRequest) -> ExitCode {
    if let Ok(stream) = UnixStream::connect(ipc::socket_path()) {
        return reply_and_exit(stream, &request);
    }

    // No daemon listening — clean up a stale socket file, if any, then spawn a
    // fresh detached daemon and wait for it to come up.
    let _ = std::fs::remove_file(ipc::socket_path());
    spawn_daemon();

    match wait_for_daemon() {
        Some(stream) => reply_and_exit(stream, &request),
        None => {
            eprintln!("termview: daemon did not start in time");
            ExitCode::FAILURE
        }
    }
}

fn spawn_daemon() {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            eprintln!("termview: failed to resolve current executable: {err}");
            return;
        }
    };

    let mut command = Command::new(exe);
    command
        .arg("__daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Detach fully from the invoking shell/terminal session so the daemon
    // survives the calling terminal closing.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    if let Err(err) = command.spawn() {
        eprintln!("termview: failed to spawn daemon: {err}");
    }
}

fn wait_for_daemon() -> Option<UnixStream> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Ok(stream) = UnixStream::connect(ipc::socket_path()) {
            return Some(stream);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn reply_and_exit(stream: UnixStream, request: &DaemonRequest) -> ExitCode {
    let mut write_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("termview: failed to talk to daemon: {err}");
            return ExitCode::FAILURE;
        }
    };

    if ipc::write_message(&mut write_stream, request).is_err() {
        eprintln!("termview: failed to send request to daemon");
        return ExitCode::FAILURE;
    }

    let mut reader = ipc::buf_reader(stream);
    match ipc::read_message::<_, DaemonResponse>(&mut reader) {
        Ok(DaemonResponse::Ok) => ExitCode::SUCCESS,
        Ok(DaemonResponse::Err(message)) => {
            eprintln!("termview: {message}");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("termview: failed to read daemon response: {err}");
            ExitCode::FAILURE
        }
    }
}
