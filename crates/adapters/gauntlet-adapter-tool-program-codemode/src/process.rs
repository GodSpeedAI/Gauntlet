//! The host process: spawned per run in its own process group, with a
//! minimal environment, bounded line reads on a thread, and bounded stderr
//! capture for diagnostics only.
//!
//! The reader threads are detached on purpose. A grandchild that outlives the
//! group kill could hold the pipes open, and joining on them would hang the
//! run. The frame channel is the only synchronization the run waits on for
//! protocol traffic. The stderr thread reports its end on its own channel, and
//! a fault waits on that channel with a bound, so the diagnostic includes the
//! host's stderr whenever the pipe closes in time.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// The environment variables the host may inherit. Everything else is
/// removed, so an inherited secret never reaches the program's process.
pub(crate) const ENV_ALLOWLIST: &[&str] = &["PATH", "LANG", "LC_ALL"];

/// The stderr bytes kept for diagnostics. Anything past this is drained and
/// dropped, so the child never blocks on a full pipe.
const STDERR_CAP: usize = 16 * 1024;

/// The stderr excerpt quoted in a failure message.
const STDERR_EXCERPT: usize = 512;

/// How long a fault waits for the host's stderr pipe to reach end-of-file
/// after the host is killed. A leaked grandchild that holds the pipe open
/// cannot stretch the run past this bound.
pub(crate) const STDERR_SETTLE: Duration = Duration::from_millis(500);

/// The host command: an executable and its argv, run in `workdir`. It is
/// configuration. It is never a shell string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCommand {
    pub executable: String,
    pub args: Vec<String>,
    pub workdir: PathBuf,
}

/// One frame read from the host's stdout.
#[derive(Debug)]
pub(crate) enum Frame {
    Line(String),
    /// The line is longer than the cap. Nothing past the cap was kept.
    TooLong,
    /// The host closed its stdout with no partial line pending.
    End,
    IoError(String),
}

/// A live host process: its writer, its line reader, and its stderr sink.
pub(crate) struct Running {
    child: Child,
    pid: u32,
    input: Sender<String>,
    frames: Receiver<Frame>,
    stderr: Arc<Mutex<Vec<u8>>>,
    /// Fires (or disconnects) once the stderr capture thread has hit end of
    /// file or a read error, so every byte the pipe held is in `stderr`.
    stderr_done: Receiver<()>,
}

/// What the next frame wait produced.
pub(crate) enum Next {
    Frame(Frame),
    /// The wait ran out of time.
    Timeout,
    /// The reader thread ended without a terminal frame.
    Disconnected,
}

impl Running {
    /// Spawns the host. The child leads its own process group, so a group
    /// signal retires the whole tree.
    pub(crate) fn spawn(host: &HostCommand, line_cap: usize) -> Result<Self, String> {
        let mut command = Command::new(&host.executable);
        command
            .args(&host.args)
            .current_dir(&host.workdir)
            .env_clear()
            .envs(allowed_env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| format!("{}: {error}", host.executable))?;
        let pid = child.id();
        let (Some(stdin), Some(stdout), Some(stderr_pipe)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            kill_group(pid);
            let _ = child.kill();
            let _ = child.wait();
            return Err("the host's standard streams were not captured".to_owned());
        };
        let (input, writes) = mpsc::channel::<String>();
        thread::spawn(move || write_lines(stdin, writes));
        let (frames_tx, frames) = mpsc::channel::<Frame>();
        thread::spawn(move || read_lines(stdout, line_cap, frames_tx));
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&stderr);
        let (done_tx, stderr_done) = mpsc::channel::<()>();
        thread::spawn(move || {
            capture_stderr(stderr_pipe, sink);
            let _ = done_tx.send(());
        });
        Ok(Self {
            child,
            pid,
            input,
            frames,
            stderr,
            stderr_done,
        })
    }

    /// Queues one line for the host. A closed channel is not an error here:
    /// the host's end arrives on stdout.
    pub(crate) fn send(&self, line: String) {
        let _ = self.input.send(line);
    }

    /// Waits up to `bound` for the next frame.
    pub(crate) fn next(&self, bound: Duration) -> Next {
        match self.frames.recv_timeout(bound) {
            Ok(frame) => Next::Frame(frame),
            Err(RecvTimeoutError::Timeout) => Next::Timeout,
            Err(RecvTimeoutError::Disconnected) => Next::Disconnected,
        }
    }

    /// Waits up to `bound` for the child to exit on its own. Returns whether
    /// it did.
    pub(crate) fn wait_for_exit(&mut self, bound: Duration) -> bool {
        let until = Instant::now() + bound;
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => return true,
                Ok(None) if Instant::now() < until => thread::sleep(Duration::from_millis(5)),
                _ => return false,
            }
        }
    }

    /// Ends the host after a protocol fault and returns the diagnostic suffix
    /// for its stderr. The group is killed first, so the host's pipe writers
    /// die. Then the parent waits up to `settle` for the stderr capture to
    /// reach end of file, so bytes the host wrote before exiting are read
    /// before the excerpt is taken. If the pipe is still open after `settle`
    /// (a grandchild that escaped the group holds it), the excerpt says so
    /// and the run does not wait longer.
    pub(crate) fn stop_and_collect_stderr(mut self, settle: Duration) -> String {
        kill_group(self.pid);
        let _ = self.child.kill();
        let _ = self.child.wait();
        let closed = match self.stderr_done.recv_timeout(settle) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => true,
            Err(RecvTimeoutError::Timeout) => false,
        };
        let text = match self.stderr.lock() {
            Ok(buffer) => String::from_utf8_lossy(&buffer).trim().to_owned(),
            Err(_) => String::new(),
        };
        let excerpt: String = text.chars().take(STDERR_EXCERPT).collect();
        match (closed, excerpt.is_empty()) {
            (true, true) => String::new(),
            (true, false) => format!("; host stderr: {excerpt}"),
            (false, true) => {
                format!("; host stderr did not close within {settle:?} and captured nothing")
            }
            (false, false) => format!(
                "; host stderr did not close within {settle:?}, so this excerpt may be incomplete: {excerpt}"
            ),
        }
    }

    /// Ends the host and its group, and reaps it. Killing a group that has
    /// already exited is a no-op, so this is safe after a clean finish.
    pub(crate) fn stop(mut self) {
        kill_group(self.pid);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The environment the host sees: only the allowlisted variables that the
/// parent has set.
pub(crate) fn allowed_env() -> Vec<(&'static str, std::ffi::OsString)> {
    ENV_ALLOWLIST
        .iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (*key, value)))
        .collect()
}

/// Signals the whole process group led by `pid` with SIGKILL. The negated pid
/// is one argv word, the portable group spelling for `kill`.
pub(crate) fn kill_group(pid: u32) {
    let group = format!("-{pid}");
    let _ = Command::new("sh")
        .args(["-c", "kill -KILL \"$1\" 2>/dev/null || true", "sh", &group])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn write_lines(mut stdin: std::process::ChildStdin, lines: Receiver<String>) {
    for line in lines {
        let written = stdin
            .write_all(line.as_bytes())
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush());
        if written.is_err() {
            return;
        }
    }
}

fn read_lines(stdout: ChildStdout, cap: usize, frames: Sender<Frame>) {
    let mut reader = BufReader::new(stdout);
    let mut buffer = Vec::new();
    loop {
        let frame = match read_frame(&mut reader, cap, &mut buffer) {
            Ok(ReadFrame::Line) => match String::from_utf8(std::mem::take(&mut buffer)) {
                Ok(text) => Frame::Line(text),
                Err(_) => Frame::IoError("a host line is not UTF-8".to_owned()),
            },
            Ok(ReadFrame::TooLong) => Frame::TooLong,
            Ok(ReadFrame::End) => Frame::End,
            Err(error) => Frame::IoError(error.to_string()),
        };
        let terminal = !matches!(frame, Frame::Line(_));
        if frames.send(frame).is_err() || terminal {
            return;
        }
    }
}

fn capture_stderr(mut stderr: ChildStderr, sink: Arc<Mutex<Vec<u8>>>) {
    let mut chunk = [0u8; 4096];
    loop {
        match stderr.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(read) => {
                if let Ok(mut buffer) = sink.lock() {
                    let room = STDERR_CAP.saturating_sub(buffer.len());
                    buffer.extend_from_slice(&chunk[..read.min(room)]);
                }
            }
        }
    }
}

enum ReadFrame {
    Line,
    TooLong,
    End,
}

/// Reads one `\n`-terminated line into `out`, holding at most `max` bytes. A
/// longer line is refused before the buffer grows past the cap.
fn read_frame<R: BufRead>(input: &mut R, max: usize, out: &mut Vec<u8>) -> io::Result<ReadFrame> {
    out.clear();
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(if out.is_empty() {
                ReadFrame::End
            } else {
                ReadFrame::Line
            });
        }
        let (take, terminated) = match available.iter().position(|&byte| byte == b'\n') {
            Some(index) => (index, true),
            None => (available.len(), false),
        };
        if out.len() + take > max {
            return Ok(ReadFrame::TooLong);
        }
        out.extend_from_slice(&available[..take]);
        if terminated {
            input.consume(take + 1);
            return Ok(ReadFrame::Line);
        }
        let consumed = available.len();
        input.consume(consumed);
    }
}
