# Getting started

## 1. What you need

Three files, all the **same architecture** (64-bit `x86_64` OR 32-bit `i686`):

| File | What it is | Where to get it |
|---|---|---|
| `trusttunnel-gui.exe` | This GUI | This project's [Releases](../../../releases) page |
| `trusttunnel_client.exe` | The VPN engine (does the tunneling) | TrustTunnel client releases |
| `wintun.dll` | User-space TUN driver | https://www.wintun.net (bundle in the `bin\<arch>` folder) |

You also need your **connection details** (endpoint host, addresses, username,
password, and usually a certificate). These come from whoever runs the VPN
endpoint -- typically exported as a `trusttunnel_client.toml` file. You can type
them in by hand or import that file (see [Configuration](configuration.md)).

> 64-bit vs 32-bit: a 32-bit (`i686`) build runs on both 32-bit and 64-bit
> Windows. If unsure -- especially on older Windows 7 -- use `i686`.

## 2. Windows 7 only: install the SHA-2 update

`wintun.dll` loads a signed driver. Windows 7 must have update **KB4474419**
(SHA-2 code-signing support) or the driver refuses to load and you cannot
connect. Windows 8/10/11 already have it. Install it first if you are on 7.

## 3. Put the files together

Create one folder and place all three files in it, for example:

```
C:\TrustTunnel\
  trusttunnel-gui.exe
  trusttunnel_client.exe
  wintun.dll
```

The GUI looks for `trusttunnel_client.exe` next to itself. (If you keep the
engine elsewhere, set its full path in Settings -> "Engine exe".)

## 4. First run

1. Right-click `trusttunnel-gui.exe` -> **Run as administrator**.
   (It always requests administrator rights -- the TUN driver needs them. You
   will see a UAC prompt; accept it.)
2. A **"Create a password"** dialog appears. Choose a password (at least 8
   characters) and confirm it. This password encrypts your saved settings on
   this machine -- see [Security](security.md). It is never stored anywhere; if
   you forget it you will have to re-enter your server details.
3. The main window opens with **Connect / Disconnect** buttons, a split-tunneling
   checkbox, and a status line.

## 5. Enter your server

Click **Settings...**, then either:

- **Import from .toml** -- pick the `trusttunnel_client.toml` your endpoint
  exported. All fields fill in automatically (recommended -- avoids retyping a
  long certificate). Then click **OK**.
- **Or type it in**: Server name (any label), Hostname (the TLS/SNI host),
  Addresses (one `ip:port` per line), Username, Password, Protocol (`http2` is
  the safe default), and Certificate if your endpoint requires one. Click **OK**.

See [Configuration](configuration.md) for what every field means.

## 6. Connect

Back on the main window, click **Connect**. The status goes
`connecting...` -> `connected`. All your traffic now goes through the VPN.

- **Minimize or close** hides the window to the system tray (the app keeps
  running). Double-click the tray icon to reopen it.
- **Right-click the tray icon** for Connect / Disconnect / Split toggle / Exit.
- Use **Exit** in the tray menu to actually quit (this also disconnects).

## 7. Next launches

On every later start you get a single **"Enter password"** prompt. Type your
password to unlock your saved settings, and the window opens ready to connect.

## Optional: split tunneling by country

If you want one country to go **direct** (bypass the VPN) while everything else
is tunneled, see the geoip section in [Configuration](configuration.md).
