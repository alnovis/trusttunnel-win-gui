//! GeoIP split-tunneling orchestration.
//!
//! Produces the `exclusions` list (country CIDRs to route DIRECT) that goes
//! into the engine config with `vpn_mode = "general"`. When split tunneling is
//! disabled the exclusion list is empty => full tunnel.
//!
//! Refresh strategy (mirrors Keenetic S97geoip):
//!   * on start, load the cached list -- fast, no network;
//!   * refresh from the RIR daily, but only while connected so the (possibly
//!     blocked) RIR host is reachable through the tunnel;
//!   * atomically replace the cache; keep the old list if a refresh fails.

pub mod cidr;
pub mod ripe;

use crate::config::{GeoipConfig, Paths};

/// Load the cached CIDR list for the configured country. Returns empty if the
/// cache is missing (first run before any refresh).
pub fn load_cached(cfg: &GeoipConfig) -> Vec<String> {
    let path = Paths::geoip_cache_file(&cfg.rir, &cfg.country);
    match std::fs::read_to_string(&path) {
        Ok(text) => text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(String::from)
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Download fresh data from the RIR, parse it, and atomically update the cache.
/// Returns the new CIDR list on success. Call this while the VPN is connected.
pub fn refresh(cfg: &GeoipConfig) -> Result<Vec<String>, String> {
    let url = ripe::rir_url(&cfg.rir).ok_or_else(|| format!("unknown RIR: {}", cfg.rir))?;
    let body = ripe::download(&url)?;
    let cidrs = ripe::parse_country_cidrs(&body, &cfg.country);
    if cidrs.is_empty() {
        return Err(format!("no {} IPv4 entries in {}", cfg.country, cfg.rir));
    }

    // Atomic replace: write to a temp file, then rename over the cache.
    let path = Paths::geoip_cache_file(&cfg.rir, &cfg.country);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("txt.tmp");
    std::fs::write(&tmp, cidrs.join("\n")).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;

    Ok(cidrs)
}

/// The exclusions to feed the engine: cached country CIDRs when split
/// tunneling is enabled, otherwise empty (full tunnel).
pub fn exclusions_for(cfg: &GeoipConfig) -> Vec<String> {
    if cfg.enabled {
        load_cached(cfg)
    } else {
        Vec::new()
    }
}
