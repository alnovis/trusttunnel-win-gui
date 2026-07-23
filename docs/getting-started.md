# Getting started

## 1. Download one file

Grab a single `.exe` from this project's [Releases](../../../releases) page. The
VPN engine and its driver (`wintun.dll`) are **built in** -- there is nothing
else to download or copy.

Pick the one that matches your Windows:

| Your Windows | Download |
|---|---|
| 64-bit (most machines) | `trusttunnel-gui-...-windows-x86_64.exe` |
| 32-bit | `trusttunnel-gui-...-windows-i686.exe` |

> Which one? Press Win+Pause (or Settings -> System -> About) and look at
> "System type". Most Windows 7 machines are 64-bit -> `x86_64`. The 32-bit and
> 64-bit builds are NOT interchangeable (the tunnel driver must match Windows),
> so if unsure, ask whoever gave you the file.

You also need your **connection details** (endpoint host, addresses, username,
password, and usually a certificate). These come from whoever runs the VPN
endpoint -- typically exported as a `trusttunnel_client.toml` file. You can type
them in by hand or import that file (see [Configuration](configuration.md)).

## 2. Windows 7 only: install the SHA-2 update

The tunnel uses a signed driver. Windows 7 must have update **KB4474419**
(SHA-2 code-signing support) or the driver refuses to load and you cannot
connect. Windows 8/10/11 already have it. Install it first if you are on 7.

## 3. First run

1. Put the `.exe` anywhere you like (Desktop, a folder, a USB stick -- it does
   not install anything).
2. Right-click it -> **Run as administrator**.
   (It always requests administrator rights -- the tunnel driver needs them.)
3. **Windows may warn that the publisher is unknown** -- this is expected (see
   below). Allow it to run.
4. A **"Create a password"** dialog appears. Choose a password (at least 8
   characters) and confirm it. This password encrypts your saved settings on
   this machine -- see [Security](security.md). It is never stored anywhere; if
   you forget it you will have to re-enter your server details.
5. The main window opens with **Connect / Disconnect** buttons, a split-tunneling
   checkbox, and a status line.

### "Windows protected your PC" / "Unknown publisher"

The app is not code-signed yet, so Windows shows warnings the first time:

- **SmartScreen** (blue box, "Windows protected your PC"): click
  **More info** -> **Run anyway**.
- **Browser download warning** ("...isn't commonly downloaded / may be
  dangerous"): choose **Keep**.
- **UAC** shows the publisher as **Unknown**: click **Yes** to allow.

These warnings mean "we could not verify who published this," not that anything
is wrong. If it makes you more comfortable, verify the file's SHA-256 checksum
against the one on the Releases page before running.

## 4. Enter your server

Click **Settings...**, then either:

- **Import from .toml** -- pick the `trusttunnel_client.toml` your endpoint
  exported. All fields fill in automatically (recommended -- avoids retyping a
  long certificate). Then click **OK**.
- **Or type it in**: Server name (any label), Hostname (the TLS/SNI host),
  Addresses (one `ip:port` per line), Username, Password, Protocol (`http2` is
  the safe default), and Certificate if your endpoint requires one. Click **OK**.

See [Configuration](configuration.md) for what every field means.

## 5. Connect

Back on the main window, click **Connect**. The status goes
`connecting...` -> `connected`. All your traffic now goes through the VPN.

- **Minimize or close** hides the window to the system tray (the app keeps
  running). Double-click the tray icon to reopen it.
- **Right-click the tray icon** for Connect / Disconnect / Split toggle / Exit.
- Use **Exit** in the tray menu to actually quit (this also disconnects).

## 6. Next launches

On every later start you get a single **"Enter password"** prompt. Type your
password to unlock your saved settings, and the window opens ready to connect.

## Optional: split tunneling by country

If you want one country to go **direct** (bypass the VPN) while everything else
is tunneled, see the geoip section in [Configuration](configuration.md).
