//! Portable facade over the platform kill switch. On Windows it drives the WFP
//! filters (`win::wfp`); elsewhere it is a no-op so `app` stays testable.
//!
//! Semantics: engaged == fail-closed (non-tunnel outbound blocked). The caller
//! (`app`) decides WHEN to engage: while the user wants to be connected but the
//! tunnel is currently DOWN (engine starting / crashed / reconnecting). While
//! Connected the switch is disengaged so tunneled traffic flows.

use std::net::IpAddr;

#[cfg(windows)]
pub struct KillSwitch {
    active: Option<crate::win::wfp::WfpKillSwitch>,
}

#[cfg(windows)]
impl KillSwitch {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn is_engaged(&self) -> bool {
        self.active.is_some()
    }

    /// Make the kill switch match `should`. Engages/disengages only on change.
    pub fn sync(&mut self, should: bool, endpoint_ips: &[IpAddr]) -> Result<(), String> {
        match (should, self.active.is_some()) {
            (true, false) => {
                self.active = Some(crate::win::wfp::WfpKillSwitch::engage(endpoint_ips)?);
            }
            (false, true) => {
                if let Some(k) = self.active.take() {
                    k.disengage();
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(not(windows))]
pub struct KillSwitch {
    active: bool,
}

#[cfg(not(windows))]
impl KillSwitch {
    pub fn new() -> Self {
        Self { active: false }
    }

    pub fn is_engaged(&self) -> bool {
        self.active
    }

    pub fn sync(&mut self, should: bool, _endpoint_ips: &[IpAddr]) -> Result<(), String> {
        self.active = should;
        Ok(())
    }
}

impl Default for KillSwitch {
    fn default() -> Self {
        Self::new()
    }
}
