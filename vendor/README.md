# vendor/

Holds the pinned, matching-architecture engine binaries embedded into release
builds via the `embed-engine` feature (`src/bootstrap.rs`). These files are
**not committed** -- the release workflow downloads them from the pinned
TrustTunnel client release and drops them here before building.

Layout (per architecture):

```
vendor/x86_64/   trusttunnel_client.exe  wintun.dll  LICENSE.txt  WINTUN_LICENSE.txt
vendor/i686/     trusttunnel_client.exe  wintun.dll  LICENSE.txt  WINTUN_LICENSE.txt
```

Source: `trusttunnel_client-v<VERSION>-windows-<arch>.zip` from
https://github.com/TrustTunnel/TrustTunnelClient/releases (each zip already
contains the matching `wintun.dll`). The pinned version is
`bootstrap::ENGINE_VERSION` and the `ver` in the release workflow -- keep them in
sync.

To build with an embedded engine locally, drop the two zips' contents here and:

```
cargo build --release --target x86_64-pc-windows-msvc --features embed-engine
```
