# trusttunnel-win-gui

Native Win32 tray GUI for the TrustTunnel VPN client, targeting Windows 7+.

It is a thin wrapper over the existing native engine
(`trusttunnel_client.exe` from the TrustTunnelClient repo): the GUI writes the
engine config and starts/stops the process. All TLS / TUN (Wintun) / routing is
done by the engine -- this app only provides a window, a tray icon, and an
optional geoip split-tunneling toggle.

## What it does

- Connect / Disconnect buttons and a tray icon (minimize/close hide to tray).
- Split-tunneling toggle (geoip): route one country DIRECT, everything else
  through the VPN. Implemented as `vpn_mode = "general"` + `exclusions = [country CIDRs]`
  in the engine config. Toggle OFF => full tunnel (no exclusions, no RIPE needed).
- Geoip list built the same way the Keenetic router does it: download the RIR
  delegated-extended file, filter by country + ipv4, convert address counts to
  CIDR prefixes. Cached locally; refreshed while connected.

## Architecture

```
GUI (this crate)                         Engine (TrustTunnelClient)
  settings.toml  --render-->  trusttunnel_client.toml  --read-->  trusttunnel_client.exe
  connect/disconnect  --spawn/kill-->  child process           --Wintun-->  TUN + routes
  geoip refresh  --download RIR-->  exclusions[]  ---------------^
```

- `src/config.rs`     -- app settings (server, geoip, killswitch) + paths.
- `src/toml_writer.rs`-- render `trusttunnel_client.toml` (mode general + exclusions).
- `src/engine.rs`     -- spawn/stop `trusttunnel_client.exe` (variant A: process control).
- `src/geoip/`        -- RIR download + parse + `count_to_prefix` + cache (portable, unit-tested).
- `src/app.rs`        -- orchestration (connect / disconnect / toggle / refresh).
- `src/ui/`           -- Win32 window + tray (windows-rs). Windows-only.
- `manifest/`         -- app.manifest (requireAdministrator, Win7..11, Common-Controls v6).

## Why these choices

- Engine reused, not rewritten: the DPI-resistant protocol stack (HTTP/2, QUIC,
  pinger, killswitch, PQ crypto) already exists and already builds for Windows
  (`platform/windows/vpn_easy`, `net/src/os_tunnel_win.cpp`).
- Native Win32, not egui: Windows 7 has no DX12 and OpenGL depends on GPU
  drivers that a work PC may lack. Win32 is boring but always runs.
- Split tunneling must be toggleable OFF: the RIR host may be unreachable
  without the VPN, and full tunnel is the safe default. Refresh geoip while
  connected so the (possibly blocked) RIR host is reachable through the tunnel.

## Windows 7 notes

- Wintun's driver is SHA-2 signed; Windows 7 needs update KB4474419 to load it.
- TLS: uses `ureq` + `rustls` (pure Rust) on purpose -- a bare Win7 SChannel may
  lack TLS 1.2, which would break HTTPS to the RIR host.
- Ship `trusttunnel_client.exe` (and `wintun.dll` matching the arch) next to the
  GUI, or set the engine path in settings.
- Requires administrator (manifest) for TUN.

## Build

The `windows` crate compiles only for Windows targets. On the dev machine
(macOS/Linux) the portable modules are still checkable:

```
cargo test                          # runs geoip/toml unit tests (non-Windows ok)
```

For the real build (on Windows, or cross with the MSVC/GNU toolchain):

```
cargo build --release --target x86_64-pc-windows-msvc
```

Match the engine/wintun architecture (x86_64 vs i686) to the target OS. See the
TrustTunnelClient Bamboo specs for the reference Windows toolchain.

## Status

Skeleton. Portable modules (config/geoip/toml_writer/engine) are functional and
unit-tested. The Win32 UI layer is wired but needs on-Windows iteration
(icon resource, worker thread for the blocking geoip download, a proper settings
dialog for entering server details).
