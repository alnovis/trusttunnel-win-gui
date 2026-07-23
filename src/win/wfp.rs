//! WFP (Windows Filtering Platform) kill switch. Windows-only.
//!
//! Installs fail-closed outbound filters that survive the engine process:
//! block all outbound v4+v6 EXCEPT loopback and the endpoint IPs (so the engine
//! can reconnect). Engaged only while the tunnel is DOWN but the user wants to
//! be connected -- see `killswitch` / `app` for the state logic. Not kept while
//! Connected, because block-all would also stop the tunneled app traffic that
//! the OS routes into Wintun.
//!
//! Uses a DYNAMIC WFP session: all filters are removed automatically when the
//! engine handle closes -- including if this process dies -- so a bug or crash
//! can never permanently lock the machine off the network.
#![cfg(windows)]

use std::net::IpAddr;

use windows::core::PWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::NetworkManagement::WindowsFilteringPlatform::*;

use crate::win::wide;

// Fixed sublayer identity for our filters.
const SUBLAYER_GUID: windows::core::GUID =
    windows::core::GUID::from_u128(0x7b2c9d14_9a4e_4d21_8f3a_6c1e0b5a77e2);

// RPC_C_AUTHN_WINNT
const RPC_C_AUTHN_WINNT: u32 = 10;

pub struct WfpKillSwitch {
    engine: HANDLE,
}

impl WfpKillSwitch {
    /// Install the kill-switch filters. `endpoint_ips` are permitted so the
    /// engine can (re)establish the tunnel while everything else is blocked.
    pub fn engage(endpoint_ips: &[IpAddr]) -> Result<Self, String> {
        unsafe {
            let mut session = FWPM_SESSION0::default();
            session.flags = FWPM_SESSION_FLAG_DYNAMIC;

            let mut engine = HANDLE::default();
            let r = FwpmEngineOpen0(None, RPC_C_AUTHN_WINNT, None, Some(&session), &mut engine);
            if r != 0 {
                return Err(format!("FwpmEngineOpen0 failed: {r} (need admin?)"));
            }

            let ks = WfpKillSwitch { engine };
            if let Err(e) = ks.install(endpoint_ips) {
                // Closing the dynamic engine drops any filters already added.
                let _ = FwpmEngineClose0(engine);
                return Err(e);
            }
            Ok(ks)
        }
    }

    unsafe fn install(&self, endpoint_ips: &[IpAddr]) -> Result<(), String> {
        let mut name = wide("TrustTunnel kill switch");
        let namep = PWSTR(name.as_mut_ptr());

        let mut sub = FWPM_SUBLAYER0::default();
        sub.subLayerKey = SUBLAYER_GUID;
        sub.displayData.name = namep;
        sub.weight = 0xFFFF;
        let r = FwpmSubLayerAdd0(self.engine, &sub, None);
        if r != 0 {
            return Err(format!("FwpmSubLayerAdd0 failed: {r}"));
        }

        let layers = [
            FWPM_LAYER_ALE_AUTH_CONNECT_V4,
            FWPM_LAYER_ALE_AUTH_CONNECT_V6,
        ];

        // 1. Block all outbound (lowest weight) on v4 and v6.
        for layer in layers {
            self.add_filter(layer, namep, &mut [], FWP_ACTION_BLOCK, 0)?;
        }

        // 2. Permit loopback (highest weight) on v4 and v6.
        for layer in layers {
            let mut cond = [loopback_condition()];
            self.add_filter(layer, namep, &mut cond, FWP_ACTION_PERMIT, 15)?;
        }

        // 3. Permit the endpoint IPs so the engine can reconnect. IPv4 only:
        //    our endpoint addresses are IPv4; an IPv6 endpoint would need the
        //    FWP_BYTE_ARRAY16 form (not handled here) and is uncommon.
        for ip in endpoint_ips {
            if let IpAddr::V4(v4) = ip {
                let mut cond = [remote_ipv4_condition(u32::from(*v4))];
                self.add_filter(
                    FWPM_LAYER_ALE_AUTH_CONNECT_V4,
                    namep,
                    &mut cond,
                    FWP_ACTION_PERMIT,
                    10,
                )?;
            }
        }

        Ok(())
    }

    unsafe fn add_filter(
        &self,
        layer: windows::core::GUID,
        name: PWSTR,
        conditions: &mut [FWPM_FILTER_CONDITION0],
        action: FWP_ACTION_TYPE,
        weight: u8,
    ) -> Result<(), String> {
        let mut f = FWPM_FILTER0::default();
        f.displayData.name = name;
        f.layerKey = layer;
        f.subLayerKey = SUBLAYER_GUID;
        f.weight = weight_u8(weight);
        f.numFilterConditions = conditions.len() as u32;
        f.filterCondition = if conditions.is_empty() {
            std::ptr::null_mut()
        } else {
            conditions.as_mut_ptr()
        };
        f.action.r#type = action;

        let r = FwpmFilterAdd0(self.engine, &f, None, None);
        if r != 0 {
            Err(format!("FwpmFilterAdd0 failed: {r}"))
        } else {
            Ok(())
        }
    }

    /// Remove the kill switch (closing the dynamic engine drops all filters).
    pub fn disengage(self) {
        unsafe {
            let _ = FwpmEngineClose0(self.engine);
        }
    }
}

fn weight_u8(w: u8) -> FWP_VALUE0 {
    let mut v = FWP_VALUE0::default();
    v.r#type = FWP_UINT8;
    v.Anonymous.uint8 = w;
    v
}

fn loopback_condition() -> FWPM_FILTER_CONDITION0 {
    let mut c = FWPM_FILTER_CONDITION0::default();
    c.fieldKey = FWPM_CONDITION_FLAGS;
    c.matchType = FWP_MATCH_FLAGS_ALL_SET;
    c.conditionValue.r#type = FWP_UINT32;
    c.conditionValue.Anonymous.uint32 = FWP_CONDITION_FLAG_IS_LOOPBACK;
    c
}

fn remote_ipv4_condition(ip_host_order: u32) -> FWPM_FILTER_CONDITION0 {
    let mut c = FWPM_FILTER_CONDITION0::default();
    c.fieldKey = FWPM_CONDITION_IP_REMOTE_ADDRESS;
    c.matchType = FWP_MATCH_EQUAL;
    c.conditionValue.r#type = FWP_UINT32;
    c.conditionValue.Anonymous.uint32 = ip_host_order; // WFP UINT32 IPv4 is host order
    c
}
