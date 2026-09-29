use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DaemonRequest {
    Open {
        workspace: String,
        command_string: Option<String>,
        shell: Option<String>,
        report: Option<String>,
        persistent: bool,
        width: Option<u32>,
        height: Option<u32>,
        cwd: Option<String>,
    },
    Quit,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DaemonResponse {
    Ok,
    Err(String),
}

// $TMPDIR is already per-user on macOS; the uid suffix is defense-in-depth for
// shared/multi-user setups rather than something strictly required here.
pub fn socket_path() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir().join(format!("termview-{uid}.sock"))
}

pub fn write_message<W: Write, T: Serialize>(writer: &mut W, message: &T) -> std::io::Result<()> {
    let mut line = serde_json::to_string(message)?;
    line.push('\n');
    writer.write_all(line.as_bytes())?;
    writer.flush()
}

pub fn read_message<R: BufRead, T: for<'de> Deserialize<'de>>(
    reader: &mut R,
) -> std::io::Result<T> {
    let mut line = String::new();
    reader.read_line(&mut line)?;
    serde_json::from_str(&line)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn buf_reader(stream: std::os::unix::net::UnixStream) -> BufReader<std::os::unix::net::UnixStream> {
    BufReader::new(stream)
}
