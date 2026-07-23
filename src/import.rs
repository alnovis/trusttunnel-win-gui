//! Import an exported `trusttunnel_client.toml` into `AppConfig`, prefilling the
//! settings dialog. The endpoint exports exactly this schema, so a user can
//! paste/load it instead of typing a long PEM chain and address list by hand.
//!
//! Only present fields overwrite (each is Option), so partial files are fine.
//! GeoIP/split (vpn_mode, exclusions) is app-managed and intentionally NOT
//! imported.

use serde::Deserialize;

use crate::config::AppConfig;

#[derive(Debug, Default, Deserialize)]
struct ImportedEndpoint {
    hostname: Option<String>,
    addresses: Option<Vec<String>>,
    has_ipv6: Option<bool>,
    username: Option<String>,
    password: Option<String>,
    client_random: Option<String>,
    skip_verification: Option<bool>,
    certificate: Option<String>,
    upstream_protocol: Option<String>,
    upstream_fallback_protocol: Option<String>,
    anti_dpi: Option<bool>,
    custom_sni: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ImportedTun {
    mtu_size: Option<u32>,
    change_system_dns: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct ImportedListener {
    tun: Option<ImportedTun>,
}

#[derive(Debug, Default, Deserialize)]
struct ImportedToml {
    loglevel: Option<String>,
    killswitch_enabled: Option<bool>,
    killswitch_allow_ports: Option<Vec<u16>>,
    post_quantum_group_enabled: Option<bool>,
    dns_upstreams: Option<Vec<String>>,
    endpoint: Option<ImportedEndpoint>,
    listener: Option<ImportedListener>,
}

/// Merge an exported engine config into `cfg`. Returns an error only on a TOML
/// syntax error.
pub fn import_into(text: &str, cfg: &mut AppConfig) -> Result<(), String> {
    let parsed: ImportedToml =
        toml::from_str(text).map_err(|e| format!("not a valid config file: {e}"))?;

    if let Some(v) = parsed.loglevel {
        cfg.log_level = v;
    }
    if let Some(v) = parsed.killswitch_enabled {
        cfg.killswitch_enabled = v;
    }
    if let Some(v) = parsed.killswitch_allow_ports {
        cfg.killswitch_allow_ports = v;
    }
    if let Some(v) = parsed.post_quantum_group_enabled {
        cfg.post_quantum_enabled = v;
    }
    if let Some(v) = parsed.dns_upstreams {
        cfg.server.dns_upstreams = v;
    }

    if let Some(e) = parsed.endpoint {
        let s = &mut cfg.server;
        if let Some(v) = e.hostname {
            s.hostname = v;
        }
        if let Some(v) = e.addresses {
            s.addresses = v;
        }
        if let Some(v) = e.has_ipv6 {
            s.has_ipv6 = v;
        }
        if let Some(v) = e.username {
            s.username = v;
        }
        if let Some(v) = e.password {
            s.password = v;
        }
        if let Some(v) = e.client_random {
            s.client_random = v;
        }
        if let Some(v) = e.skip_verification {
            s.skip_verification = v;
        }
        if let Some(v) = e.certificate {
            s.certificate_pem = v.trim().to_string();
        }
        if let Some(v) = e.upstream_protocol {
            s.upstream_protocol = v;
        }
        if let Some(v) = e.upstream_fallback_protocol {
            s.upstream_fallback_protocol = v;
        }
        if let Some(v) = e.anti_dpi {
            s.anti_dpi = v;
        }
        if let Some(v) = e.custom_sni {
            s.custom_sni = v;
        }
    }

    if let Some(tun) = parsed.listener.and_then(|l| l.tun) {
        if let Some(v) = tun.mtu_size {
            cfg.mtu_size = v;
        }
        if let Some(v) = tun.change_system_dns {
            cfg.change_system_dns = v;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
loglevel = "debug"
vpn_mode = "general"
killswitch_enabled = true
post_quantum_group_enabled = true
exclusions = []
dns_upstreams = ["8.8.8.8:53"]

[endpoint]
hostname = "holland.example.io"
addresses = ["1.2.3.4:443", "5.6.7.8:8443"]
has_ipv6 = true
username = "alice"
password = "secret:1"
skip_verification = false
upstream_protocol = "http2"
upstream_fallback_protocol = "http2"
anti_dpi = false
certificate = """
-----BEGIN CERTIFICATE-----
ABC
-----END CERTIFICATE-----
"""

[listener.tun]
mtu_size = 1400
change_system_dns = false
"#;

    #[test]
    fn imports_endpoint_and_toplevel() {
        let mut cfg = AppConfig::default();
        import_into(SAMPLE, &mut cfg).unwrap();
        assert_eq!(cfg.server.hostname, "holland.example.io");
        assert_eq!(cfg.server.addresses, vec!["1.2.3.4:443", "5.6.7.8:8443"]);
        assert_eq!(cfg.server.username, "alice");
        assert_eq!(cfg.server.password, "secret:1");
        assert!(cfg.server.has_ipv6);
        assert_eq!(cfg.log_level, "debug");
        assert_eq!(cfg.server.dns_upstreams, vec!["8.8.8.8:53"]);
        assert_eq!(cfg.mtu_size, 1400);
        assert!(!cfg.change_system_dns);
        assert!(cfg.server.certificate_pem.starts_with("-----BEGIN CERTIFICATE-----"));
    }

    #[test]
    fn partial_file_keeps_defaults() {
        let mut cfg = AppConfig::default();
        import_into("[endpoint]\nhostname = \"h\"\n", &mut cfg).unwrap();
        assert_eq!(cfg.server.hostname, "h");
        assert_eq!(cfg.server.upstream_protocol, "http2"); // default kept
    }

    #[test]
    fn garbage_errors() {
        let mut cfg = AppConfig::default();
        assert!(import_into("this is not toml = = =", &mut cfg).is_err());
    }
}
