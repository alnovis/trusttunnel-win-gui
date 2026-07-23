//! Best-effort secure delete: overwrite the file contents with random bytes,
//! flush, then remove. NOTE: on SSDs / journaling / copy-on-write filesystems
//! overwrite-in-place does NOT guarantee the old bytes are gone (wear
//! leveling, snapshots). This shrinks the exposure, it is not a guarantee --
//! the real protection is keeping plaintext on disk only briefly.

use std::io::Write;
use std::path::Path;

pub fn shred_file(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let len = meta.len() as usize;
        if len > 0 {
            if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(path) {
                let mut buf = vec![0u8; len.min(64 * 1024)];
                let _ = getrandom::getrandom(&mut buf);
                let mut remaining = len;
                while remaining > 0 {
                    let n = remaining.min(buf.len());
                    if f.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    remaining -= n;
                }
                let _ = f.sync_all();
            }
        }
    }
    let _ = std::fs::remove_file(path);
}
