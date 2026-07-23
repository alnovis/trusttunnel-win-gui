//! Small Win32 helpers. Windows-only.
#![cfg(windows)]

pub mod acl;
pub mod proc;
pub mod wfp;

/// Convert a Rust string to a NUL-terminated UTF-16 buffer for the wide Win32
/// APIs (CreateWindowExW, etc.).
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
