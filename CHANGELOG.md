# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/) and this project follows
[Semantic Versioning](https://semver.org/). The release workflow reads the
section matching the pushed tag (`## [X.Y.Z]`) as the GitHub Release notes.

## [Unreleased]

## [0.1.3] - 2026-07-24

### Added
- Show the app version (from Cargo.toml) in the window title, the main window,
  and the tray tooltip -- so it is obvious which build is running (a stale
  instance in the tray otherwise just gets re-surfaced on the next launch).

## [0.1.2] - 2026-07-24

### Fixed
- TUN mode reported "connected" but no traffic reached the tunnel: the generated
  engine config omitted `included_routes`/`excluded_routes`, so the Wintun
  adapter came up with no routes and nothing was sent into it. Now emits
  `0.0.0.0/0` + `2000::/3`, keeping LAN/reserved ranges direct. Affected every
  platform, not only Windows 7.

### Added
- Pre-commit hook (`cargo fmt --check`) under `.githooks`; enable with
  `git config core.hooksPath .githooks`.

## [0.1.1] - 2026-07-24

### Added
- Listener-mode switch (TUN / SOCKS) in Settings -> Advanced, so the engine can
  run as a SOCKS5 proxy on 127.0.0.1:1080 with no Wintun driver -- useful where
  the TUN data plane is unavailable.

### Fixed
- Build warnings: removed genuinely dead code and silenced the host-only
  dead-code false positives.

## [0.1.0] - 2026-07-23

Initial release.

### Added
- Native Win32 tray GUI wrapping the TrustTunnel engine: connect / disconnect,
  minimize and close to the system tray.
- GeoIP split tunneling (route one country direct, everything else through the
  VPN), toggleable; per-RIR list download, cached and refreshable.
- Auto-reconnect watchdog driven by the engine's stdout state plus an external
  egress probe; adopts an engine that outlived a previous instance;
  single-instance guard.
- Settings dialog with server details and an Advanced screen; import from an
  exported `trusttunnel_client.toml`; change-password flow.
- Encrypted settings at rest (passphrase, Argon2id + XChaCha20-Poly1305). The
  plaintext engine config lives on disk only briefly, ACL-locked and then
  shredded. WFP fail-closed kill switch.
- Single self-contained exe: the matching-architecture engine and wintun.dll are
  embedded and extracted on first run.
- Builds for Windows 7 through 11 (win7 targets) in x86_64 and i686; the release
  workflow publishes both with SHA-256 checksums.
