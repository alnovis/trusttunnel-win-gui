# Troubleshooting

## "trusttunnel_client.exe not found"

The engine is built into the exe and normally unpacks itself on first run, so
this is rare. It can happen if you set a custom **Engine exe** path in Settings
that is wrong (clear it to use the built-in engine), or if the app could not
write to `%ProgramData%\TrustTunnel\bin`. Run as administrator and try again.
If you did point it at your own engine, make sure its architecture matches
Windows (x86_64 vs i686).

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

- On **Windows 7**: confirm update **KB4474419** is installed. Without it the
  bundled tunnel driver cannot load and the tunnel never comes up.
- Make sure you downloaded the build that matches Windows (x86_64 for 64-bit,
  i686 for 32-bit) -- a mismatched build cannot load the driver.
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
