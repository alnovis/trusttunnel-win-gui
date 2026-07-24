//! Render the engine config (`trusttunnel_client.toml`) from `AppConfig` plus a
//! resolved exclusions list. The engine (`trusttunnel_client.exe`) reads this
//! file; see TrustTunnelClient/trusttunnel/README.md for the schema.
//!
//! Split-tunneling policy: `vpn_mode = "general"` (everything through the VPN
//! except `exclusions`). With an empty exclusions list this is a full tunnel.

use crate::config::AppConfig;

/// Build the TOML text. `exclusions` are country CIDRs to route direct.
pub fn render(cfg: &AppConfig, exclusions: &[String]) -> String {
    let s = &cfg.server;
    let mut out = String::new();

    out.push_str(&format!("loglevel = {}\n", quote(&cfg.log_level)));
    // Always general: full tunnel when exclusions is empty, split when not.
    out.push_str("vpn_mode = \"general\"\n");
    out.push_str(&format!(
        "killswitch_enabled = {}\n",
        cfg.killswitch_enabled
    ));
    if !cfg.killswitch_allow_ports.is_empty() {
        let ports: Vec<String> = cfg
            .killswitch_allow_ports
            .iter()
            .map(|p| p.to_string())
            .collect();
        out.push_str(&format!(
            "killswitch_allow_ports = [{}]\n",
            ports.join(", ")
        ));
    }
    out.push_str(&format!(
        "post_quantum_group_enabled = {}\n",
        cfg.post_quantum_enabled
    ));
    out.push_str(&format!("exclusions = {}\n", str_array(exclusions)));
    out.push_str(&format!(
        "dns_upstreams = {}\n",
        str_array(&s.dns_upstreams)
    ));
    out.push('\n');

    out.push_str("[endpoint]\n");
    out.push_str(&format!("hostname = {}\n", quote(&s.hostname)));
    out.push_str(&format!("addresses = {}\n", str_array(&s.addresses)));
    out.push_str(&format!("username = {}\n", quote(&s.username)));
    out.push_str(&format!("password = {}\n", quote(&s.password)));
    out.push_str(&format!("has_ipv6 = {}\n", s.has_ipv6));
    out.push_str(&format!("anti_dpi = {}\n", s.anti_dpi));
    out.push_str(&format!("skip_verification = {}\n", s.skip_verification));
    out.push_str(&format!(
        "upstream_protocol = {}\n",
        quote(&s.upstream_protocol)
    ));
    if !s.upstream_fallback_protocol.is_empty() {
        out.push_str(&format!(
            "upstream_fallback_protocol = {}\n",
            quote(&s.upstream_fallback_protocol)
        ));
    }
    if !s.client_random.is_empty() {
        out.push_str(&format!("client_random = {}\n", quote(&s.client_random)));
    }
    if !s.custom_sni.is_empty() {
        out.push_str(&format!("custom_sni = {}\n", quote(&s.custom_sni)));
    }
    if !s.certificate_pem.is_empty() {
        // Multi-line PEM as a TOML triple-quoted string.
        out.push_str("certificate = \"\"\"\n");
        out.push_str(s.certificate_pem.trim());
        out.push_str("\n\"\"\"\n");
    }
    out.push('\n');

    // Listener: SOCKS5 proxy (no driver) or system-wide TUN (Wintun).
    if cfg.listener_mode == "socks" {
        out.push_str("[listener.socks]\n");
        out.push_str(&format!("address = {}\n", quote(&cfg.socks_address)));
    } else {
        // TUN. included_routes/excluded_routes MUST be present -- the engine
        // only programs routes it finds in the config (absent => empty => the
        // adapter comes up but nothing is routed into it => "connected" but no
        // traffic). Route everything in; keep LAN/reserved ranges direct.
        out.push_str("[listener.tun]\n");
        out.push_str("included_routes = [\"0.0.0.0/0\", \"2000::/3\"]\n");
        out.push_str(
            "excluded_routes = [\"0.0.0.0/8\", \"10.0.0.0/8\", \"169.254.0.0/16\", \
             \"172.16.0.0/12\", \"192.168.0.0/16\", \"224.0.0.0/3\"]\n",
        );
        out.push_str(&format!("mtu_size = {}\n", cfg.mtu_size));
        out.push_str(&format!("change_system_dns = {}\n", cfg.change_system_dns));
    }

    out
}

fn quote(s: &str) -> String {
    // Minimal TOML basic-string escaping.
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn str_array(items: &[String]) -> String {
    if items.is_empty() {
        return "[]".into();
    }
    let inner: Vec<String> = items.iter().map(|i| quote(i)).collect();
    format!("[{}]", inner.join(", "))
}

/// Render and write the engine config to its canonical path (atomically).
pub fn write_engine_config(
    cfg: &AppConfig,
    exclusions: &[String],
) -> std::io::Result<std::path::PathBuf> {
    use crate::config::Paths;
    let path = Paths::engine_config_file();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    write_atomic(&path, render(cfg, exclusions).as_bytes())?;
    Ok(path)
}

/// Write `bytes` to `path` atomically: write a sibling temp file, fsync it, then
/// rename over the target. A watchdog restart or a crash can never observe a
/// half-written config -- the reader sees either the old file or the new one.
/// `std::fs::rename` replaces an existing destination on both Windows
/// (MoveFileExW) and Unix, and is atomic within a filesystem.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    // Create the parent dir if missing (e.g. first run, before %APPDATA%\TrustTunnel exists).
    std::fs::create_dir_all(dir)?;
    let stem = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".to_string());
    // Per-process temp name so concurrent writers do not collide.
    let tmp = dir.join(format!("{stem}.{}.tmp", std::process::id()));

    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?; // durable on disk before the rename
    }

    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    #[test]
    fn full_tunnel_has_empty_exclusions() {
        let cfg = AppConfig::default();
        let toml = render(&cfg, &[]);
        assert!(toml.contains("vpn_mode = \"general\""));
        assert!(toml.contains("exclusions = []"));
    }

    #[test]
    fn split_emits_cidrs() {
        let cfg = AppConfig::default();
        let toml = render(&cfg, &["1.2.3.0/24".into(), "5.6.0.0/16".into()]);
        assert!(toml.contains("exclusions = [\"1.2.3.0/24\", \"5.6.0.0/16\"]"));
    }

    #[test]
    fn tun_mode_emits_tun_listener() {
        let cfg = AppConfig::default(); // default listener_mode = "tun"
        let toml = render(&cfg, &[]);
        assert!(toml.contains("[listener.tun]"));
        assert!(!toml.contains("[listener.socks]"));
        // The routes are what actually send traffic into the tunnel; without
        // included_routes the engine connects but routes nothing.
        assert!(toml.contains("included_routes = [\"0.0.0.0/0\", \"2000::/3\"]"));
        assert!(toml.contains("excluded_routes = ["));
    }

    #[test]
    fn socks_mode_emits_socks_listener() {
        let mut cfg = AppConfig::default();
        cfg.listener_mode = "socks".into();
        cfg.socks_address = "127.0.0.1:1080".into();
        let toml = render(&cfg, &[]);
        assert!(toml.contains("[listener.socks]"));
        assert!(toml.contains("address = \"127.0.0.1:1080\""));
        assert!(!toml.contains("[listener.tun]"));
    }

    #[test]
    fn atomic_write_replaces_and_leaves_no_temp() {
        let dir = std::env::temp_dir().join(format!("ttwin_atomic_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("engine.toml");

        super::write_atomic(&path, b"first").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "first");
        super::write_atomic(&path, b"second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");

        // No leftover *.tmp in the directory.
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp file left behind");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn atomic_write_creates_missing_parent_dir() {
        // First-run case: the target's parent directory does not exist yet.
        let base = std::env::temp_dir().join(format!("ttwin_mkdir_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let path = base.join("nested").join("settings.enc");
        super::write_atomic(&path, b"payload").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"payload");
        let _ = std::fs::remove_dir_all(&base);
    }
}
