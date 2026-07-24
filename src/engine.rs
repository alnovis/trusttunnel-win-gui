//! Variant A: drive the existing `trusttunnel_client.exe` as an independent
//! child process. The engine SURVIVES a wrapper crash (no Job Object); a
//! restarted wrapper rediscovers it via the PID file (see `adopt_existing`).
//!
//! stdout+stderr are piped into a channel so `engine_state::StateTracker` can
//! derive the connection state, and the watchdog can act on it.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::pidfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStatus {
    Stopped,
    Running,
}

pub struct Engine {
    exe: PathBuf,
    /// Process we spawned this session (has piped stdout).
    owned: Option<Child>,
    /// Process discovered from the PID file (no stdout access).
    adopted_pid: Option<u32>,
    /// Merged stdout/stderr lines from the owned process.
    lines: Option<Receiver<String>>,
    /// True once we asked the engine to stop (distinguishes clean vs crash).
    requested_stop: bool,
}

impl Engine {
    pub fn new(exe_hint: &str) -> Self {
        let exe = if exe_hint.is_empty() {
            default_engine_path()
        } else {
            PathBuf::from(exe_hint)
        };
        Self {
            exe,
            owned: None,
            adopted_pid: None,
            lines: None,
            requested_stop: false,
        }
    }

    pub fn exe_exists(&self) -> bool {
        self.exe.is_file()
    }

    pub fn requested_stop(&self) -> bool {
        self.requested_stop
    }

    /// On wrapper startup: if the PID file points at a live trusttunnel_client
    /// process, adopt it so the UI shows "connected" and the watchdog can
    /// supervise it. Returns true if a process was adopted.
    pub fn adopt_existing(&mut self) -> bool {
        let Some(pid) = pidfile::read() else {
            return false;
        };
        let exe_name = self
            .exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("trusttunnel_client.exe");
        if pid_is_engine(pid, exe_name) {
            self.adopted_pid = Some(pid);
            self.requested_stop = false;
            true
        } else {
            // Stale PID file (process gone or PID reused) -- clean it up.
            pidfile::clear();
            false
        }
    }

    pub fn status(&mut self) -> EngineStatus {
        if let Some(c) = self.owned.as_mut() {
            return match c.try_wait() {
                Ok(Some(_)) => {
                    self.owned = None;
                    EngineStatus::Stopped
                }
                Ok(None) => EngineStatus::Running,
                Err(_) => EngineStatus::Stopped,
            };
        }
        if let Some(pid) = self.adopted_pid {
            if pid_is_alive(pid) {
                return EngineStatus::Running;
            }
            self.adopted_pid = None;
        }
        EngineStatus::Stopped
    }

    /// Non-blocking poll of the next engine log line (owned process only).
    pub fn next_line(&mut self) -> Option<String> {
        match self.lines.as_ref()?.try_recv() {
            Ok(line) => Some(line),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.lines = None;
                None
            }
        }
    }

    pub fn start(&mut self, config_path: &Path, log_level: &str) -> std::io::Result<()> {
        self.stop();
        self.requested_stop = false;

        let mut cmd = std::process::Command::new(&self.exe);
        cmd.arg("--config").arg(config_path);
        cmd.arg("--loglevel").arg(log_level);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn()?;
        let pid = child.id();

        // Merge stdout + stderr into one line channel.
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        if let Some(out) = child.stdout.take() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }
        if let Some(err) = child.stderr.take() {
            std::thread::spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
            });
        }

        self.owned = Some(child);
        self.adopted_pid = None;
        self.lines = Some(rx);
        let _ = pidfile::write(pid);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.requested_stop = true;
        if let Some(mut c) = self.owned.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if let Some(pid) = self.adopted_pid.take() {
            terminate_pid(pid);
        }
        self.lines = None;
        pidfile::clear();
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Note: on a hard crash Drop does not run, which is exactly why the
        // engine is independent and rediscovered via the PID file.
        self.stop();
    }
}

fn default_engine_path() -> PathBuf {
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    dir.push("trusttunnel_client.exe");
    dir
}

// --- platform-specific process checks (real impl in win::proc) ---

fn pid_is_engine(pid: u32, exe_name: &str) -> bool {
    #[cfg(windows)]
    {
        crate::win::proc::pid_alive_and_named(pid, exe_name)
    }
    #[cfg(not(windows))]
    {
        let _ = (pid, exe_name);
        false // cannot verify off-Windows; treat as no adoptable process
    }
}

fn pid_is_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        crate::win::proc::pid_alive(pid)
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        false
    }
}

fn terminate_pid(pid: u32) {
    #[cfg(windows)]
    {
        crate::win::proc::terminate(pid);
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
    }
}
