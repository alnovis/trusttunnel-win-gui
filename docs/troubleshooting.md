# Troubleshooting

## "trusttunnel_client.exe not found"

The GUI cannot find the engine. Either put `trusttunnel_client.exe` in the same
folder as `trusttunnel-gui.exe`, or set its full path in
Settings -> **Engine exe**. Make sure its architecture matches the GUI
(both x86_64, or both i686).

## Status shows FAILED right after connecting

The engine started but hit a non-retryable error. The status line shows the
reason:

- **authentication failed** -- wrong username/password.
- **certificate error** -- the endpoint certificate did not verify. Import the
  correct certificate, or (if you understand the risk) tick "Skip certificate
  verification".
- **bad configuration** -- a required field is missing or malformed (check
  Hostname and Addresses).

Fix the setting and click Connect again. The app does not retry a fatal error in
a loop.

## Stuck on "connecting..." / never connects

- On **Windows 7**: confirm update **KB4474419** is installed. Without it
  `wintun.dll` cannot load its driver and the tunnel never comes up.
- Confirm `wintun.dll` is present next to the engine and matches the
  architecture.
- Some networks block **QUIC**. If your Protocol is `http3`, switch to `http2`
  in Settings.
- Check the addresses are reachable; try raising Log level to `debug`.

## The window disappeared

Minimize and close hide the app to the **system tray**, they do not quit it.
Double-click the tray icon to reopen the window, or right-click it for the menu.
Use **Exit** in the tray menu to actually quit.

## "Refresh geoip list" fails

The country IP list is downloaded from an internet registry (RIPE/ARIN/...),
which your local network may block. **Connect first, then refresh** -- while
connected the download goes through the tunnel. If the registry is unreachable
even through the VPN, the previous cached list (if any) keeps working.

## Wrong country is bypassing / not bypassing the VPN

Check the **RIR** matches the **Country** (for example `RU` is under `ripencc`,
US IPs are under `arin`). After changing them, reconnect and Refresh the geoip
list.

## Forgot the password

There is no recovery. Delete `%APPDATA%\TrustTunnel\settings.enc`, restart, set
a new password, and re-enter or re-import your server details.

## UAC prompt every launch

Expected -- the app requires administrator rights for the TUN driver. There is
no way around it while running a system-wide tunnel.

## Nothing happens when I double-click the exe

- A second copy just brings the first instance's window to the front (only one
  instance runs at a time). Check the tray.
- If you declined the UAC prompt, it exits. Run it again and accept.
