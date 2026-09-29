use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "termview")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Open (or reuse) a terminal session in a named workspace.
    Open(OpenArgs),
    /// Stop the background daemon and all its sessions.
    Quit,
    /// Internal: run as the background daemon. Not for direct use.
    #[command(name = "__daemon", hide = true)]
    Daemon,
}

#[derive(Parser, Debug, Clone)]
pub struct OpenArgs {
    /// The command to run, as a single shell-quoted string, e.g. "yazi ~/projects".
    /// Always run directly (never wrapped in a shell). When omitted, opens an
    /// interactive session in the resolved shell instead (see --shell).
    pub command_string: Option<String>,

    /// Workspace name — identifies this session. Reusing a workspace with the same
    /// command reattaches to the existing session; a different command restarts it.
    #[arg(short, long)]
    pub workspace: String,

    /// Shell to use when command_string is omitted, e.g. "nu". Overrides
    /// config.toml's `shell` for this invocation. Falls back to config.toml's
    /// `shell`, then $SHELL, then /bin/zsh when unset.
    #[arg(long)]
    pub shell: Option<String>,

    /// URL template fired via `open` when the program exits on its own. `{stdout}`
    /// is replaced with the (URL-encoded) final on-screen terminal text.
    #[arg(long)]
    pub report: Option<String>,

    /// Keep the session running in the background when the window is closed
    /// (Cmd+W). Without this flag, closing the window kills the session.
    #[arg(long, default_value_t = false)]
    pub persistent: bool,

    /// Override the window width for this invocation.
    #[arg(long)]
    pub width: Option<u32>,

    /// Override the window height for this invocation.
    #[arg(long)]
    pub height: Option<u32>,

    /// Working directory for the launched program (tilde-expanded).
    #[arg(long)]
    pub cwd: Option<String>,
}
