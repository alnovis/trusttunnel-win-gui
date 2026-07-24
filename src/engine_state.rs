//! Parse the engine's stdout/stderr into a connection state.
//!
//! Markers come from trusttunnel_client.cpp (get_state_changed_callback):
//!   * "Successfully connected to endpoint"       -> Connected
//!   * "Waiting recovery: to next=..ms error=.."  -> engine self-recovering
//!   * "Error: <code> <text>" (VPN_SS_DISCONNECTED with error) then the engine
//!     calls stop and the PROCESS EXITS -- so a fatal error == process exit.
//!   * "Failed to verify certificate: .."         -> fatal (cert)
//!   * "Failed parsing configuration" / "Failed to parse config" -> fatal (config)
//!
//! We match on message substrings, not the log prefix, so timestamp/level
//! formatting does not matter.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailReason {
    Auth,
    Certificate,
    Config,
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnState {
    /// No engine running / user has not asked to connect.
    Idle,
    /// Engine started, not yet connected.
    Connecting,
    /// "Successfully connected to endpoint".
    Connected,
    /// Engine reported it is self-recovering ("Waiting recovery"). Do NOT
    /// force-restart here -- the engine handles it (pinger/fallback/addresses).
    Reconnecting,
    /// Engine exited without a recognized fatal cause -> watchdog should retry.
    Crashed,
    /// Engine hit a non-retryable error (bad creds/cert/config). Surface to the
    /// user; do NOT auto-retry.
    Failed(FailReason),
    /// Normal shutdown at our request.
    Disconnected,
}

impl ConnState {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnState::Connected)
    }
}

/// One meaningful signal extracted from a log line.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Signal {
    Connected,
    Reconnecting,
    Fatal(FailReason),
}

fn classify(line: &str) -> Option<Signal> {
    let l = line.to_ascii_lowercase();
    if l.contains("successfully connected to endpoint") {
        return Some(Signal::Connected);
    }
    if l.contains("waiting recovery") {
        return Some(Signal::Reconnecting);
    }
    if l.contains("failed to verify certificate") {
        return Some(Signal::Fatal(FailReason::Certificate));
    }
    if l.contains("failed parsing configuration") || l.contains("failed to parse config") {
        return Some(Signal::Fatal(FailReason::Config));
    }
    if l.contains("failed to start network monitor") {
        return Some(Signal::Fatal(FailReason::Network));
    }
    // Generic "Error: <code> <text>" from a disconnect-with-error; classify by
    // keywords in the text.
    if l.contains("error:") || l.starts_with("error ") {
        if l.contains("cert") {
            return Some(Signal::Fatal(FailReason::Certificate));
        }
        if l.contains("auth") || l.contains("credential") || l.contains("unauthorized") {
            return Some(Signal::Fatal(FailReason::Auth));
        }
        // An error we cannot classify: not necessarily fatal, leave state alone
        // (the process exit handler decides retry vs give-up).
    }
    None
}

/// Accumulates log lines into a live connection state.
pub struct StateTracker {
    pub state: ConnState,
    pub last_error: Option<String>,
    /// Last recognized fatal reason (persists across the process-exit).
    fatal: Option<FailReason>,
}

impl StateTracker {
    pub fn new() -> Self {
        Self {
            state: ConnState::Idle,
            last_error: None,
            fatal: None,
        }
    }

    /// Call when we (re)start the engine.
    pub fn on_start(&mut self) {
        self.state = ConnState::Connecting;
        self.fatal = None;
        self.last_error = None;
    }

    /// Feed one stdout/stderr line.
    pub fn ingest(&mut self, line: &str) {
        match classify(line) {
            Some(Signal::Connected) => self.state = ConnState::Connected,
            Some(Signal::Reconnecting) => self.state = ConnState::Reconnecting,
            Some(Signal::Fatal(reason)) => {
                self.fatal = Some(reason);
                self.last_error = Some(line.trim().to_string());
            }
            None => {
                // Capture bare "Error:" lines for display even if unclassified.
                if line.to_ascii_lowercase().contains("error") {
                    self.last_error = Some(line.trim().to_string());
                }
            }
        }
    }

    /// Call when the engine process exits.
    /// `requested_stop` = true when WE asked it to stop (normal shutdown).
    pub fn on_process_exit(&mut self, requested_stop: bool) {
        self.state = if requested_stop {
            ConnState::Disconnected
        } else if let Some(reason) = self.fatal {
            ConnState::Failed(reason)
        } else {
            ConnState::Crashed
        };
    }
}

impl Default for StateTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_then_recover_then_connect() {
        let mut t = StateTracker::new();
        t.on_start();
        assert_eq!(t.state, ConnState::Connecting);
        t.ingest("[info] Successfully connected to endpoint");
        assert_eq!(t.state, ConnState::Connected);
        t.ingest("[info] Waiting recovery: to next=2000ms error=0 ");
        assert_eq!(t.state, ConnState::Reconnecting);
        t.ingest("2026-07-23 12:00:00 [info] Successfully connected to endpoint");
        assert_eq!(t.state, ConnState::Connected);
    }

    #[test]
    fn fatal_cert_survives_exit() {
        let mut t = StateTracker::new();
        t.on_start();
        t.ingest("[error] Failed to verify certificate: expired");
        t.on_process_exit(false);
        assert_eq!(t.state, ConnState::Failed(FailReason::Certificate));
    }

    #[test]
    fn generic_auth_error_is_fatal() {
        let mut t = StateTracker::new();
        t.on_start();
        t.ingest("[error] Error: 6 authentication required");
        t.on_process_exit(false);
        assert_eq!(t.state, ConnState::Failed(FailReason::Auth));
    }

    #[test]
    fn unexpected_exit_is_crashed_not_failed() {
        let mut t = StateTracker::new();
        t.on_start();
        t.ingest("[info] Successfully connected to endpoint");
        t.on_process_exit(false);
        assert_eq!(t.state, ConnState::Crashed);
    }

    #[test]
    fn requested_stop_is_clean() {
        let mut t = StateTracker::new();
        t.on_start();
        t.ingest("[info] Successfully connected to endpoint");
        t.on_process_exit(true);
        assert_eq!(t.state, ConnState::Disconnected);
    }
}
