//! Engine PID file, so a restarted wrapper can rediscover an engine it did not
//! spawn (variant A: engine survives a wrapper crash). Aliveness + image-name
//! verification is platform-specific -- see `win::proc`.

use crate::config::Paths;
use std::path::PathBuf;

fn path() -> PathBuf {
    Paths::program_data_dir().join("engine.pid")
}

pub fn write(pid: u32) -> std::io::Result<()> {
    let p = path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(p, pid.to_string())
}

pub fn read() -> Option<u32> {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

pub fn clear() {
    let _ = std::fs::remove_file(path());
}
