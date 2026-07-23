# Configuration

Open the settings with the **Settings...** button on the main window. There are
two screens -- the main one and **Advanced...** -- plus **Import from .toml** and
**Change password...**.

## Importing an exported config

If your endpoint gave you a `trusttunnel_client.toml`, click **Import from
.toml**, pick the file, and the server fields fill in automatically (hostname,
addresses, username, password, certificate, protocol, DNS, and the advanced
fields). This is the easiest and least error-prone way to configure a server.
Split-tunneling settings are NOT imported -- they stay as you set them.

## Main settings

| Field | Meaning |
|---|---|
| Name | Display label only; does not affect the connection. |
| Hostname | The endpoint's TLS/SNI host name. |
| Addresses | One `ip:port` per line. The engine pings them and picks the best. |
| Username / Password | Your endpoint credentials. |
| Protocol | `http2` (HTTP/2) or `http3` (QUIC). Use `http2` unless told otherwise -- some networks block QUIC. |
| Fallback | Protocol to try if the main one fails. `(none)` to disable. |
| Certificate (PEM) | The endpoint certificate. Leave empty to trust the system store; paste the PEM if your endpoint uses its own. |
| Skip certificate verification | Accept any certificate. Insecure -- leave off unless you know why. |
| Enable split tunneling (geoip) | See below. |
| RIR / Country | Which registry and country the geoip list is for (see below). |
| Refresh (hours) | How often to auto-refresh the geoip list. |
| Kill switch | Block traffic from leaking directly while the tunnel is down. |
| Log level | `error` / `warn` / `info` / `debug` / `trace`. Raise it only for troubleshooting. |
| Engine exe | Full path to `trusttunnel_client.exe`. Leave empty to use the one next to the GUI. |

## Advanced settings

| Field | Meaning |
|---|---|
| Route IPv6 through VPN | Send IPv6 traffic through the tunnel too. |
| Anti-DPI | Enable the engine's anti-DPI measures. |
| Post-quantum crypto | Use a post-quantum key exchange in TLS (on by default). |
| Let engine change system DNS | Allow the engine to set the system DNS while connected. |
| client_random | TLS client-random prefix `prefix[/mask]` (advanced auth). |
| custom_sni | Override the SNI separately from the hostname. |
| MTU size | Tunnel MTU (default 1280). |
| DNS upstreams | One per line; DNS resolvers to use through the tunnel. |
| Kill switch allow ports | Comma-separated inbound ports allowed while the kill switch is on. |

## Split tunneling (geoip)

By default the client is a **full tunnel** -- everything goes through the VPN.
Split tunneling lets you send **one country's IP ranges directly** (bypassing
the VPN) while everything else stays tunneled. This mirrors how the router
setup routes local-country traffic straight to the ISP.

To enable it:

1. In Settings, tick **Enable split tunneling (geoip)** and set **Country** (an
   ISO code such as `RU`) and **RIR** (the internet registry that covers that
   country: `ripencc` for Europe/CIS, `arin`, `apnic`, `lacnic`, `afrinic`).
2. Click **OK**.
3. **Connect first**, then click **Refresh geoip list** on the main window.

Why refresh while connected: the address list is downloaded from the registry
(RIPE/ARIN/...), which may be unreachable on your local network but is reachable
through the tunnel. Downloading while connected routes the request through the
VPN. The list is cached, so later launches do not need to re-download.

Turning the checkbox **off** returns you to a full tunnel immediately.

## Changing the password

Settings -> **Change password...**. Enter your current password, then the new
one twice. The change takes effect immediately (it re-encrypts your settings
file), independent of whether you click OK or Cancel on the settings screen.

## Where files live

| File | Location |
|---|---|
| Encrypted settings | `%APPDATA%\TrustTunnel\settings.enc` |
| Engine config (temporary) | `%ProgramData%\TrustTunnel\trusttunnel_client.toml` |
| GeoIP cache | `%ProgramData%\TrustTunnel\geoip_<rir>_<country>.txt` |

The engine config holds your credentials in plain text only for the ~1.5 seconds
the engine needs to read it at startup, then it is shredded. See
[Security](security.md).
