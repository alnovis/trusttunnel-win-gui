//! Win32 UI layer. Windows-only.
#![cfg(windows)]

pub mod dialog;
pub mod tray;
pub mod window;

// Child control ids (WM_COMMAND wParam low word).
// Single connect/disconnect toggle button (owner-drawn: colour + label reflect
// the connection state -- a plain Win32 push button cannot be recoloured).
pub const IDC_TOGGLE: i32 = 1001;
pub const IDC_SPLIT: i32 = 1003;
pub const IDC_REFRESH: i32 = 1004;
pub const IDC_STATUS: i32 = 1005;
pub const IDC_SETTINGS: i32 = 1006;

// Tray context-menu command ids.
pub const IDM_SHOW: u32 = 2001;
pub const IDM_CONNECT: u32 = 2002;
pub const IDM_DISCONNECT: u32 = 2003;
pub const IDM_SPLIT: u32 = 2004;
pub const IDM_EXIT: u32 = 2005;

// Private window messages.
pub const WM_APP_TRAY: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
/// Posted by the geoip worker thread when a refresh finishes.
pub const WM_APP_GEOIP_DONE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;

// Tray icon id.
pub const TRAY_UID: u32 = 1;
