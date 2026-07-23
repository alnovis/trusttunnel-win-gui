// Embeds the application manifest (requireAdministrator + Win7 supportedOS +
// Common-Controls v6) and the tray/app icon into the final executable.
//
// Keyed on the TARGET os (via CARGO_CFG_TARGET_OS), not the host, so it also
// runs when cross-building the Windows target from macOS/Linux. embed-resource
// picks the right tool (MSVC rc on Windows, windres for the GNU target).
fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        embed_resource::compile("manifest/app.rc", embed_resource::NONE);
    }
}
