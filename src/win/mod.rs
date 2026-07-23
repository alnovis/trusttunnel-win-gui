//! Small Win32 helpers. Windows-only.
#![cfg(windows)]

pub mod acl;
pub mod proc;
pub mod wfp;

/// The integer type SetWindowLongPtrW / GetWindowLongPtrW use for a
/// GWLP_USERDATA slot: `isize` on 64-bit Windows, `i32` on 32-bit (where the
/// *Ptr* APIs alias the non-Ptr ones). Cast pointer-sized values with this so
/// the code builds for both x86_64 and i686.
#[cfg(target_pointer_width = "64")]
pub type WinLong = isize;
#[cfg(target_pointer_width = "32")]
pub type WinLong = i32;

/// Convert a Rust string to a NUL-terminated UTF-16 buffer for the wide Win32
/// APIs (CreateWindowExW, etc.).
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
