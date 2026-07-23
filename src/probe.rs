//! External connectivity probe through the tunnel.
//!
//! When the engine runs a full/general tunnel, a plain outbound request from
//! this process is already routed through Wintun by the OS -- so we do NOT bind
//! to the TUN adapter. The only requirement is that the probe target is NOT in
//! the excluded (geoip) country, otherwise it would go direct and tell us
//! nothing about the tunnel. Pick a definitely-foreign target.
//!
//! This is the second detection layer (after engine stdout state): it catches
//! "engine alive and thinks it is connected, but egress is dead".

use std::time::Duration;

/// Small, fast, fixed-response endpoints that return HTTP 204. Foreign hosts so
/// they route through the VPN (not into the geoip-excluded country).
pub const DEFAULT_PROBE_URLS: &[&str] = &[
    "https://www.gstatic.com/generate_204",
    "https://connectivitycheck.gstatic.com/generate_204",
    "https://1.1.1.1/cdn-cgi/trace",
];

pub struct ProbeConfig {
    pub urls: Vec<String>,
    pub timeout: Duration,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            urls: DEFAULT_PROBE_URLS.iter().map(|s| s.to_string()).collect(),
            timeout: Duration::from_secs(5),
        }
    }
}

/// True if ANY probe URL is reachable. Tries them in order; first success wins.
/// A 204/2xx is success. Network/timeout/refused is failure.
pub fn probe_ok(cfg: &ProbeConfig) -> bool {
    for url in &cfg.urls {
        match ureq::get(url).timeout(cfg.timeout).call() {
            Ok(resp) => {
                let code = resp.status();
                if code == 204 || (200..300).contains(&code) {
                    return true;
                }
            }
            // ureq returns Err on non-2xx too; treat 3xx as reachable enough.
            Err(ureq::Error::Status(code, _)) if (200..400).contains(&code) => return true,
            Err(_) => continue,
        }
    }
    false
}

// --- background probe worker ---

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// Runs `probe_ok` on a cadence, off the UI thread. The UI timer reads results
/// via `latest()` and gates probing via `set_active()`. Probing is only enabled
/// while we believe the tunnel is connected -- so a self-recovering engine
/// (Reconnecting) is never probed and cannot produce false failures.
pub struct ProbeWorker {
    active: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    rx: Receiver<bool>,
}

impl ProbeWorker {
    /// Spawn the worker. `interval` is the gap between probes (e.g. 20s); each
    /// probe itself blocks up to `cfg.timeout` per URL.
    pub fn spawn(cfg: ProbeConfig, interval: Duration) -> Self {
        let active = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let (tx, rx) = channel::<bool>();

        let a = active.clone();
        let s = stop.clone();
        std::thread::spawn(move || {
            let step = Duration::from_millis(500);
            let mut elapsed = Duration::ZERO;
            loop {
                if s.load(Ordering::Relaxed) {
                    break;
                }
                std::thread::sleep(step);
                elapsed += step;
                if elapsed < interval {
                    continue;
                }
                elapsed = Duration::ZERO;
                if a.load(Ordering::Relaxed) {
                    let ok = probe_ok(&cfg);
                    if tx.send(ok).is_err() {
                        break; // receiver gone
                    }
                }
            }
        });

        Self { active, stop, rx }
    }

    /// Enable/disable probing (set true only while connected).
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }

    /// Latest probe result since the last call, if any (drains the queue).
    pub fn latest(&self) -> Option<bool> {
        let mut last = None;
        while let Ok(v) = self.rx.try_recv() {
            last = Some(v);
        }
        last
    }
}

impl Drop for ProbeWorker {
    fn drop(&mut self) {
        // Signal the thread to exit; it is detached, so we do not join (a probe
        // in flight would otherwise block shutdown up to its timeout).
        self.stop.store(true, Ordering::Relaxed);
    }
}
