//! Optional embedded-engine bootstrap.
//!
//! With the `embed-engine` feature (release builds), the matching-architecture
//! `trusttunnel_client.exe` + `wintun.dll` are baked into this exe and extracted
//! to `%ProgramData%\TrustTunnel\bin` on first run, so the user needs only this
//! single file. Without the feature (dev builds / non-Windows) it is a no-op and
//! the engine comes from the Settings path or a sibling `trusttunnel_client.exe`.
//!
//! Note: the engine architecture must match the OS (Wintun is a kernel driver),
//! so each release asset embeds its own arch's engine + wintun.dll.

use crate::config::Paths;

/// Pinned TrustTunnel client release the embedded binaries come from.
/// Keep in sync with the version the release workflow downloads.
pub const ENGINE_VERSION: &str = "1.0.23";

#[cfg(all(
    feature = "embed-engine",
    target_os = "windows",
    target_arch = "x86_64"
))]
mod embedded {
    pub const AVAILABLE: bool = true;
    pub const ENGINE: &[u8] = include_bytes!("../vendor/x86_64/trusttunnel_client.exe");
    pub const WINTUN: &[u8] = include_bytes!("../vendor/x86_64/wintun.dll");
    pub const ENGINE_LICENSE: &[u8] = include_bytes!("../vendor/x86_64/LICENSE.txt");
    pub const WINTUN_LICENSE: &[u8] = include_bytes!("../vendor/x86_64/WINTUN_LICENSE.txt");
}

#[cfg(all(feature = "embed-engine", target_os = "windows", target_arch = "x86"))]
mod embedded {
    pub const AVAILABLE: bool = true;
    pub const ENGINE: &[u8] = include_bytes!("../vendor/i686/trusttunnel_client.exe");
    pub const WINTUN: &[u8] = include_bytes!("../vendor/i686/wintun.dll");
    pub const ENGINE_LICENSE: &[u8] = include_bytes!("../vendor/i686/LICENSE.txt");
    pub const WINTUN_LICENSE: &[u8] = include_bytes!("../vendor/i686/WINTUN_LICENSE.txt");
}

#[cfg(not(all(
    feature = "embed-engine",
    target_os = "windows",
    any(target_arch = "x86_64", target_arch = "x86")
)))]
mod embedded {
    pub const AVAILABLE: bool = false;
    pub const ENGINE: &[u8] = &[];
    pub const WINTUN: &[u8] = &[];
    pub const ENGINE_LICENSE: &[u8] = &[];
    pub const WINTUN_LICENSE: &[u8] = &[];
}

/// If an engine is embedded, extract it (plus wintun.dll and the licenses) to a
/// private bin dir on first run / version change, and return the engine exe
/// path. Returns None when nothing is embedded -- the caller then falls back to
/// the Settings path or a sibling exe.
pub fn ensure_engine() -> Option<String> {
    if !embedded::AVAILABLE {
        return None;
    }

    let dir = Paths::program_data_dir().join("bin");
    let engine = dir.join("trusttunnel_client.exe");
    let wintun = dir.join("wintun.dll");
    let marker = dir.join(".engine-version");

    let up_to_date = engine.exists()
        && wintun.exists()
        && std::fs::read_to_string(&marker).ok().as_deref() == Some(ENGINE_VERSION);

    if !up_to_date {
        if std::fs::create_dir_all(&dir).is_err() {
            return None;
        }
        // A previously-adopted engine may still hold the exe open; if a write
        // fails but a usable file is already present, we fall through below.
        let _ = std::fs::write(&engine, embedded::ENGINE);
        let _ = std::fs::write(&wintun, embedded::WINTUN);
        let _ = std::fs::write(
            dir.join("TRUSTTUNNEL_LICENSE.txt"),
            embedded::ENGINE_LICENSE,
        );
        let _ = std::fs::write(dir.join("WINTUN_LICENSE.txt"), embedded::WINTUN_LICENSE);
        let _ = std::fs::write(&marker, ENGINE_VERSION);
    }

    if engine.exists() {
        Some(engine.to_string_lossy().into_owned())
    } else {
        None
    }
}
