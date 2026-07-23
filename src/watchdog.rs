//! Watchdog supervisor: drives the engine's ACTUAL state toward the user's
//! DESIRED state. Tick-based (the UI/engine layer calls `tick` on a timer) so
//! the decision logic stays portable and unit-testable -- no threads or clocks
//! in here.
//!
//! Two detection layers feed it:
//!   1. engine stdout state (`engine_state::ConnState`) -- primary;
//!   2. external probe through the tunnel (`probe`) -- secondary, catches a
//!      "connected but egress dead" hang.
//!
//! Guard rails:
//!   * NEVER act on `Reconnecting` -- the engine self-heals (pinger / fallback
//!     protocol / multiple addresses); force-restarting there fights it.
//!   * `Failed(reason)` is fatal (bad creds/cert/config) -> give up, surface to
//!     the user, do not loop.
//!   * A restart budget caps crash/probe-restart churn -> give up if exceeded.

use crate::engine_state::ConnState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desired {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Nothing to do this tick.
    None,
    /// Stop+start the engine (crash recovery or stuck-egress recovery).
    Restart,
    /// Stop trying; surface the failure to the user.
    GiveUp,
}

pub struct Params {
    /// Consecutive failed probes before we treat egress as dead.
    pub probe_fail_threshold: usize,
    /// Max restarts within the tracking window before giving up (stops an
    /// infinite crash-restart loop even for non-fatal exits).
    pub max_restarts: usize,
}

impl Default for Params {
    fn default() -> Self {
        Self { probe_fail_threshold: 3, max_restarts: 5 }
    }
}

/// Mutable counters the supervisor maintains between ticks.
#[derive(Debug, Default)]
pub struct Counters {
    pub probe_fails: usize,
    pub restarts: usize,
}

/// Pure decision: given desired vs actual state and current counters, what to
/// do this tick. Does not mutate.
pub fn decide(desired: Desired, state: &ConnState, c: &Counters, p: &Params) -> Action {
    if desired == Desired::Disconnected {
        return Action::None;
    }
    // desired == Connected
    match state {
        // Fatal: never auto-retry.
        ConnState::Failed(_) => Action::GiveUp,

        // Engine is handling recovery itself; stay out of its way.
        ConnState::Reconnecting | ConnState::Connecting => Action::None,

        // Engine gone/unexpected while we want it up -> restart within budget.
        ConnState::Crashed | ConnState::Disconnected | ConnState::Idle => {
            if c.restarts >= p.max_restarts {
                Action::GiveUp
            } else {
                Action::Restart
            }
        }

        // Connected: only the probe layer can trigger a restart here.
        ConnState::Connected => {
            if c.probe_fails >= p.probe_fail_threshold {
                if c.restarts >= p.max_restarts {
                    Action::GiveUp
                } else {
                    Action::Restart
                }
            } else {
                Action::None
            }
        }
    }
}

/// Record a probe result into the counters (call each probe tick while
/// Connected). A success clears the fail streak.
pub fn record_probe(c: &mut Counters, ok: bool) {
    if ok {
        c.probe_fails = 0;
    } else {
        c.probe_fails += 1;
    }
}

/// Reset the restart budget -- call after a sustained healthy period.
pub fn note_healthy(c: &mut Counters) {
    c.restarts = 0;
    c.probe_fails = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_state::FailReason;

    fn p() -> Params {
        Params::default()
    }

    #[test]
    fn disconnected_desire_is_passive() {
        let c = Counters::default();
        assert_eq!(decide(Desired::Disconnected, &ConnState::Crashed, &c, &p()), Action::None);
    }

    #[test]
    fn crash_restarts_within_budget() {
        let c = Counters { restarts: 2, ..Default::default() };
        assert_eq!(decide(Desired::Connected, &ConnState::Crashed, &c, &p()), Action::Restart);
    }

    #[test]
    fn crash_gives_up_over_budget() {
        let c = Counters { restarts: 5, ..Default::default() };
        assert_eq!(decide(Desired::Connected, &ConnState::Crashed, &c, &p()), Action::GiveUp);
    }

    #[test]
    fn fatal_always_gives_up() {
        let c = Counters::default();
        assert_eq!(
            decide(Desired::Connected, &ConnState::Failed(FailReason::Auth), &c, &p()),
            Action::GiveUp
        );
    }

    #[test]
    fn reconnecting_is_left_alone() {
        let c = Counters { probe_fails: 9, ..Default::default() };
        assert_eq!(decide(Desired::Connected, &ConnState::Reconnecting, &c, &p()), Action::None);
    }

    #[test]
    fn connected_healthy_does_nothing() {
        let c = Counters { probe_fails: 0, ..Default::default() };
        assert_eq!(decide(Desired::Connected, &ConnState::Connected, &c, &p()), Action::None);
    }

    #[test]
    fn connected_dead_egress_restarts() {
        let c = Counters { probe_fails: 3, ..Default::default() };
        assert_eq!(decide(Desired::Connected, &ConnState::Connected, &c, &p()), Action::Restart);
    }

    #[test]
    fn probe_streak_clears_on_success() {
        let mut c = Counters { probe_fails: 2, ..Default::default() };
        record_probe(&mut c, true);
        assert_eq!(c.probe_fails, 0);
    }
}
