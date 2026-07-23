# trusttunnel-win-gui -- documentation

User guide for the Windows tray client.

- [Getting started](getting-started.md) -- what you need, download, file layout,
  first run, connecting.
- [Configuration](configuration.md) -- every setting explained; importing an
  exported config; geoip split tunneling.
- [Security](security.md) -- the password/encryption model, the kill switch, and
  what is (and isn't) protected on a machine you don't control.
- [Troubleshooting](troubleshooting.md) -- common problems and fixes.

## In one paragraph

`trusttunnel-gui.exe` is a thin GUI over the TrustTunnel VPN engine
(`trusttunnel_client.exe`). You drop the GUI, the engine, and `wintun.dll`
into one folder, run the GUI as administrator, set a password, enter (or
import) your server details, and click Connect. The engine does the actual
tunneling; the GUI manages it, keeps it reconnected, and encrypts your settings
at rest.
