//! Orchestration shared by the UI: owns settings + engine + the watchdog
//! supervisor. Turns user actions into config writes / engine restarts, and
//! reconciles ACTUAL state (engine stdout + probe) toward DESIRED state.
//!
//! `tick` is non-blocking and portable (unit-testable). The UI/engine layer
//! calls it on a timer and supplies an optional probe result gathered off the
//! UI thread.

use std::net::IpAddr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use crate::config::{AppConfig, GeoipConfig, Paths};
use crate::engine::{Engine, EngineStatus};
use crate::engine_state::{ConnState, StateTracker};
use crate::killswitch::KillSwitch;
use crate::probe::{ProbeConfig, ProbeWorker};
use crate::secret::Vault;
use crate::watchdog::{self, Action, Counters, Desired, Params};
use crate::{geoip, shred, toml_writer};

/// The engine config is plaintext; delete it this soon after the engine has
/// started (it parses the file once at startup, then keeps it in memory).
const ENGINE_CONFIG_TTL: Duration = Duration::from_millis(1500);

/// How often the background worker probes egress while connected.
const PROBE_INTERVAL: Duration = Duration::from_secs(20);

pub struct App {
    pub cfg: AppConfig,
    /// Unlocked vault used to re-seal settings on save. Updated in place by the
    /// change-password flow in the settings dialog.
    pub(crate) vault: Vault,
    pub engine: Engine,
    /// When the current engine process was started (for plaintext-config TTL).
    engine_started_at: Option<Instant>,
    tracker: StateTracker,
    desired: Desired,
    counters: Counters,
    params: Params,
    probe: ProbeWorker,
    /// Fail-closed kill switch, engaged while the tunnel is down but wanted.
    killswitch: KillSwitch,
    /// Result channel for an in-flight geoip refresh (runs off the UI thread).
    geoip_rx: Option<Receiver<Result<usize, String>>>,
    refreshing: bool,
}

/// Handed to a worker thread to run one geoip refresh. The UI layer spawns the
/// thread (so the windows-specific wake-up / PostMessage stays out of App).
pub struct GeoipRefreshJob {
    pub geoip: GeoipConfig,
    pub tx: Sender<Result<usize, String>>,
}

impl App {
    /// Built after the settings vault is unlocked (see `ui::window::run`).
    pub fn new(vault: Vault, cfg: AppConfig) -> Self {
        // Engine path: an explicit Settings path wins (manual override);
        // otherwise use the embedded engine (extracted on first run) if present;
        // otherwise a sibling trusttunnel_client.exe (Engine::new default).
        let engine_path = if !cfg.engine_exe.is_empty() {
            cfg.engine_exe.clone()
        } else {
            crate::bootstrap::ensure_engine().unwrap_or_default()
        };
        let mut engine = Engine::new(&engine_path);

        // Rediscover an engine that outlived a previous wrapper instance.
        let mut tracker = StateTracker::new();
        let mut desired = Desired::Disconnected;
        if engine.adopt_existing() {
            // We cannot read an adopted process's stdout, so assume it is up
            // and let the probe/liveness layers correct us.
            tracker.state = ConnState::Connected;
            desired = Desired::Connected;
        }

        let probe = ProbeWorker::spawn(ProbeConfig::default(), PROBE_INTERVAL);

        Self {
            cfg,
            vault,
            engine,
            engine_started_at: None,
            tracker,
            desired,
            counters: Counters::default(),
            params: Params::default(),
            probe,
            killswitch: KillSwitch::new(),
            geoip_rx: None,
            refreshing: false,
        }
    }

    /// Encrypt the current settings and write settings.enc atomically.
    pub fn save_settings(&self) -> Result<(), String> {
        let blob = self.vault.seal(self.cfg.to_toml().as_bytes())?;
        toml_writer::write_atomic(&Paths::settings_file(), &blob)
            .map_err(|e| format!("save settings: {e}"))
    }

    /// Driver called on the UI timer: gate the probe worker by connection
    /// state, pull its latest result, and run one watchdog tick. Keeps the
    /// blocking probe off the UI thread while `tick` stays pure/testable.
    pub fn service_tick(&mut self) -> ConnState {
        let probing = self.desired == Desired::Connected && self.tracker.state.is_connected();
        self.probe.set_active(probing);
        let probe_result = if probing { self.probe.latest() } else { None };
        self.tick(probe_result)
    }

    // --- introspection for the UI ---

    pub fn state(&self) -> &ConnState {
        &self.tracker.state
    }

    pub fn last_error(&self) -> Option<&str> {
        self.tracker.last_error.as_deref()
    }

    pub fn is_connected(&mut self) -> bool {
        self.engine.status() == EngineStatus::Running
    }

    pub fn split_enabled(&self) -> bool {
        self.cfg.geoip.enabled
    }

    // --- user actions ---

    pub fn connect(&mut self) -> Result<(), String> {
        if !self.engine.exe_exists() {
            return Err("trusttunnel_client.exe not found (set its path in settings)".into());
        }
        self.desired = Desired::Connected;
        watchdog::note_healthy(&mut self.counters);
        let r = self.start_engine();
        self.update_killswitch();
        r
    }

    pub fn disconnect(&mut self) {
        self.desired = Desired::Disconnected;
        self.engine.stop();
        self.tracker.state = ConnState::Disconnected;
        self.engine_started_at = None;
        shred::shred_file(&Paths::engine_config_file());
        watchdog::note_healthy(&mut self.counters);
        self.update_killswitch();
    }

    pub fn set_split_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.cfg.geoip.enabled = enabled;
        self.save_settings()?;
        if self.desired == Desired::Connected {
            self.start_engine()?;
        }
        Ok(())
    }

    /// Begin an async geoip refresh. Returns a job for the UI layer to run on a
    /// worker thread, or None if a refresh is already in flight. Refresh is best
    /// done WHILE connected so a blocked RIR host is reachable through the tunnel.
    pub fn begin_geoip_refresh(&mut self) -> Option<GeoipRefreshJob> {
        if self.refreshing {
            return None;
        }
        self.refreshing = true;
        let (tx, rx) = channel();
        self.geoip_rx = Some(rx);
        Some(GeoipRefreshJob {
            geoip: self.cfg.geoip.clone(),
            tx,
        })
    }

    /// Finalize a completed refresh (called on the UI thread after the worker
    /// signals done). Reapplies if split is on and connected. Returns a status
    /// string for the UI.
    pub fn finish_geoip_refresh(&mut self) -> String {
        self.refreshing = false;
        let result = self.geoip_rx.take().and_then(|rx| rx.try_recv().ok());
        match result {
            Some(Ok(n)) => {
                if self.cfg.geoip.enabled && self.desired == Desired::Connected {
                    if let Err(e) = self.start_engine() {
                        return format!("Geoip updated ({n} CIDRs) but reapply failed: {e}");
                    }
                }
                format!("Geoip updated: {n} CIDRs")
            }
            Some(Err(e)) => format!("Geoip error: {e}"),
            None => "Geoip refresh: no result".to_string(),
        }
    }

    pub fn is_refreshing(&self) -> bool {
        self.refreshing
    }

    // --- watchdog tick ---

    /// Reconcile actual toward desired. `probe_result` is an optional external
    /// connectivity probe (Some(true)=egress ok) gathered off the UI thread.
    /// Returns the current connection state for display.
    pub fn tick(&mut self, probe_result: Option<bool>) -> ConnState {
        // 1. Ingest any new engine log lines.
        while let Some(line) = self.engine.next_line() {
            self.tracker.ingest(&line);
        }

        // 2. Detect process exit.
        if self.engine.status() == EngineStatus::Stopped {
            match self.tracker.state {
                // Already terminal -- leave it.
                ConnState::Failed(_) | ConnState::Crashed | ConnState::Disconnected => {}
                _ => self.tracker.on_process_exit(self.engine.requested_stop()),
            }
        }

        // 3. Fold in a probe result while we believe we are connected.
        if let Some(ok) = probe_result {
            if self.tracker.state.is_connected() {
                watchdog::record_probe(&mut self.counters, ok);
            }
        }

        // 4. Decide and act.
        match watchdog::decide(
            self.desired,
            &self.tracker.state,
            &self.counters,
            &self.params,
        ) {
            Action::Restart => {
                self.counters.restarts += 1;
                self.counters.probe_fails = 0;
                let _ = self.start_engine();
            }
            Action::GiveUp => {
                // Latch off; the user must fix settings and reconnect.
                self.desired = Desired::Disconnected;
                self.engine.stop();
                self.engine_started_at = None;
                shred::shred_file(&Paths::engine_config_file());
            }
            Action::None => {}
        }

        // Delete the plaintext engine config once the engine has read it.
        self.maybe_shred_engine_config();

        // Reconcile the kill switch with the (possibly just-changed) state.
        self.update_killswitch();

        self.tracker.state.clone()
    }

    /// Engage the fail-closed kill switch while the user wants to be connected
    /// but the tunnel is currently down (starting / crashed / reconnecting);
    /// disengage once Connected (so tunneled traffic flows) or when the user
    /// disconnects. Gated by the killswitch_enabled setting. Best-effort: a WFP
    /// failure must not block VPN operation.
    fn update_killswitch(&mut self) {
        let should = self.cfg.killswitch_enabled
            && self.desired == Desired::Connected
            && !self.tracker.state.is_connected();
        let ips = if should {
            self.endpoint_ips()
        } else {
            Vec::new()
        };
        if let Err(e) = self.killswitch.sync(should, &ips) {
            self.tracker.last_error = Some(format!("kill switch: {e}"));
        }
    }

    /// Endpoint IPs parsed from the configured addresses ("ip:port" / "ip").
    /// Hostnames are skipped (cannot be permitted without resolving).
    fn endpoint_ips(&self) -> Vec<IpAddr> {
        use std::str::FromStr;
        let mut out = Vec::new();
        for a in &self.cfg.server.addresses {
            if let Ok(sa) = std::net::SocketAddr::from_str(a) {
                out.push(sa.ip());
            } else if let Ok(ip) = IpAddr::from_str(a) {
                out.push(ip);
            }
        }
        out
    }

    /// Persist settings edited via the dialog and, if connected, reapply them
    /// by restarting the engine with the new config.
    pub fn apply_settings(&mut self) -> Result<(), String> {
        self.save_settings()?;
        if self.desired == Desired::Connected {
            self.start_engine()?;
        }
        Ok(())
    }

    // --- internal ---

    fn start_engine(&mut self) -> Result<(), String> {
        let exclusions = geoip::exclusions_for(&self.cfg.geoip);
        let path = toml_writer::write_engine_config(&self.cfg, &exclusions)
            .map_err(|e| format!("write config: {e}"))?;
        // Lock the plaintext config to SYSTEM+Administrators for its brief life.
        #[cfg(windows)]
        crate::win::acl::restrict_file(&path);
        self.engine
            .start(&path, &self.cfg.log_level)
            .map_err(|e| format!("start engine: {e}"))?;
        self.engine_started_at = Some(Instant::now());
        self.tracker.on_start();
        Ok(())
    }

    /// Delete the plaintext engine config once the engine has had time to parse
    /// it (shrinking on-disk credential exposure to ~ENGINE_CONFIG_TTL).
    fn maybe_shred_engine_config(&mut self) {
        if let Some(started) = self.engine_started_at {
            if self.engine.status() == EngineStatus::Running
                && started.elapsed() >= ENGINE_CONFIG_TTL
            {
                shred::shred_file(&Paths::engine_config_file());
                self.engine_started_at = None;
            }
        }
    }
}
