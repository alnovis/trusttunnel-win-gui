// Native Win32 tray GUI for the TrustTunnel VPN client (Windows 7+).
//
// Thin wrapper over the existing trusttunnel_client.exe engine: connect /
// disconnect, minimize to tray, and an optional geoip split-tunneling toggle.
// The engine does all TLS/TUN/routing; this app only writes config and manages
// the process (see src/app.rs).
#![cfg_attr(windows, windows_subsystem = "windows")]
// On non-Windows the entry point is a stub, so the whole App/UI call graph is
// unreachable and every item looks dead. Silence that only off-Windows; real
// dead-code detection stays active for the actual (Windows) target.
#![cfg_attr(not(windows), allow(dead_code))]

mod app;
mod bootstrap;
mod config;
mod engine;
mod engine_state;
mod geoip;
mod import;
mod killswitch;
mod pidfile;
mod probe;
mod secret;
mod shred;
mod toml_writer;
mod watchdog;

#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    // Single-instance guard. Two wrappers would fight over the Wintun adapter
    // and the engine config file, so a second launch just surfaces the first
    // instance's window and exits. The guard must outlive the message loop.
    let _guard = match win::proc::acquire_single_instance("Global\\TrustTunnelGuiSingleton") {
        Some(g) => g,
        None => {
            ui::window::activate_existing();
            return;
        }
    };

    // Unlock (or first-run create) the encrypted settings before starting.
    // Cancelling the password prompt exits without running.
    let (vault, cfg) = match ui::dialog::unlock_or_create() {
        Some(x) => x,
        None => return,
    };
    ui::window::run(vault, cfg);
}

// The `windows` crate is Windows-only; on the dev machine (macOS/Linux) we can
// still `cargo test` the portable modules (geoip/config/toml_writer/engine).
#[cfg(not(windows))]
fn main() {
    eprintln!(
        "trusttunnel-gui targets Windows. Portable modules are unit-tested via `cargo test`."
    );
}
