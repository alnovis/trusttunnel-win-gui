//! Application settings (what the user configures in the GUI) and filesystem
//! paths. This is NOT the engine config -- see `toml_writer` for the
//! `trusttunnel_client.toml` that is handed to `trusttunnel_client.exe`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Endpoint connection parameters -- the subset a user must enter, mirroring
/// the exported `trusttunnel_client.toml` `[endpoint]` section.
///
/// `#[serde(default)]` so a settings.toml written by an older build (missing
/// the advanced fields) still loads.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    // --- MVP ---
    /// Display label, app-only.
    pub name: String,
    /// TLS/SNI host (`[endpoint] hostname`).
    pub hostname: String,
    /// "ip:port" or "host:port"; the pinger picks the best one.
    pub addresses: Vec<String>,
    pub username: String,
    pub password: String,
    /// "http2" | "http3".
    pub upstream_protocol: String,
    /// Optional fallback protocol; empty = none.
    pub upstream_fallback_protocol: String,
    /// PEM chain; empty = verify against the system store.
    pub certificate_pem: String,
    pub skip_verification: bool,
    /// DNS upstreams routed through the tunnel; empty = endpoint default.
    pub dns_upstreams: Vec<String>,

    // --- Advanced ---
    /// Route IPv6 through the endpoint (`[endpoint] has_ipv6`).
    pub has_ipv6: bool,
    /// Enable anti-DPI measures (`[endpoint] anti_dpi`).
    pub anti_dpi: bool,
    /// TLS client random prefix `prefix[/mask]` (`[endpoint] client_random`).
    pub client_random: String,
    /// Override SNI separately from hostname (`[endpoint] custom_sni`, 1.0.3+).
    pub custom_sni: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "My server".into(),
            hostname: String::new(),
            addresses: Vec::new(),
            username: String::new(),
            password: String::new(),
            upstream_protocol: "http2".into(),
            upstream_fallback_protocol: String::new(),
            certificate_pem: String::new(),
            skip_verification: false,
            dns_upstreams: Vec::new(),
            has_ipv6: false,
            anti_dpi: false,
            client_random: String::new(),
            custom_sni: String::new(),
        }
    }
}

/// GeoIP split-tunneling settings. When `enabled` is false the app runs a full
/// tunnel (no exclusions) -- see the module docs on why the toggle matters
/// (RIPE may be unreachable without the VPN up).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeoipConfig {
    /// Master switch for split tunneling. false => full tunnel.
    pub enabled: bool,
    /// RIR delegated-stats source: "ripencc" | "arin" | "apnic" | "lacnic" | "afrinic".
    pub rir: String,
    /// ISO country code to route DIRECT (bypass VPN), e.g. "RU".
    pub country: String,
    /// Refresh cadence in hours (Keenetic uses daily => 24).
    pub refresh_hours: u64,
}

impl Default for GeoipConfig {
    fn default() -> Self {
        Self {
            enabled: false, // safe default: full tunnel, no RIPE dependency
            rir: "ripencc".into(),
            country: "RU".into(),
            refresh_hours: 24,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub geoip: GeoipConfig,
    pub killswitch_enabled: bool,
    /// "error" | "warn" | "info" | "debug" | "trace".
    pub log_level: String,
    /// Absolute path to trusttunnel_client.exe. Empty => look next to the GUI.
    pub engine_exe: String,

    // --- Advanced ---
    /// Post-quantum key exchange in TLS (`post_quantum_group_enabled`).
    pub post_quantum_enabled: bool,
    /// TUN MTU (`[listener.tun] mtu_size`).
    pub mtu_size: u32,
    /// Let the engine set system DNS (`[listener.tun] change_system_dns`).
    pub change_system_dns: bool,
    /// Ports allowed inbound while the kill switch is on (`killswitch_allow_ports`).
    pub killswitch_allow_ports: Vec<u16>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            geoip: GeoipConfig::default(),
            killswitch_enabled: true,
            log_level: "info".into(),
            engine_exe: String::new(),
            post_quantum_enabled: true,
            mtu_size: 1280,
            change_system_dns: true,
            killswitch_allow_ports: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Serialize to TOML (the plaintext that gets encrypted at rest).
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("serialize settings")
    }

    /// Parse from TOML (the decrypted plaintext).
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| format!("settings parse error: {e}"))
    }
}

/// Filesystem locations. Per-user settings live in %APPDATA%; machine-wide
/// engine config + geoip cache live in %ProgramData% (readable by the elevated
/// engine process).
pub struct Paths;

impl Paths {
    pub fn app_data_dir() -> PathBuf {
        // %APPDATA%\TrustTunnel
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("TrustTunnel")
    }

    pub fn program_data_dir() -> PathBuf {
        // %ProgramData%\TrustTunnel
        let base = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("TrustTunnel")
    }

    /// Encrypted settings vault.
    pub fn settings_file() -> PathBuf {
        Self::app_data_dir().join("settings.enc")
    }

    /// Legacy plaintext settings, migrated + shredded on first encrypted run.
    pub fn legacy_settings_file() -> PathBuf {
        Self::app_data_dir().join("settings.toml")
    }

    /// The generated engine config handed to trusttunnel_client.exe.
    pub fn engine_config_file() -> PathBuf {
        Self::program_data_dir().join("trusttunnel_client.toml")
    }

    /// Cached geoip CIDR list (one CIDR per line), keyed by rir+country.
    pub fn geoip_cache_file(rir: &str, country: &str) -> PathBuf {
        Self::program_data_dir().join(format!("geoip_{rir}_{country}.txt"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_roundtrip() {
        let cfg = AppConfig::default();
        let text = cfg.to_toml();
        assert!(AppConfig::from_toml(&text).is_ok());
    }

    #[test]
    fn from_toml_rejects_garbage() {
        // The unlock self-check relies on this: decrypted-but-invalid content
        // must be an error, never silently accepted.
        assert!(AppConfig::from_toml("this is = = not valid toml =").is_err());
        assert!(AppConfig::from_toml("\u{0}\u{1}\u{2}binary junk").is_err());
    }
}
