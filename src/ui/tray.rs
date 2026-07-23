//! System tray icon + context menu via Shell_NotifyIcon.
#![cfg(windows)]

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, POINT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, LoadIconW, SetForegroundWindow,
    TrackPopupMenu, HICON, IDI_APPLICATION, MF_SEPARATOR, MF_STRING, TPM_BOTTOMALIGN,
    TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
};

use crate::win::wide;

fn base_nid(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut nid = NOTIFYICONDATAW::default();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = super::TRAY_UID;
    nid
}

fn app_icon() -> HICON {
    // Embedded app.ico (resource id 1); fall back to the system icon.
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).map(Into::into).unwrap_or_default();
        LoadIconW(hinst, PCWSTR(1usize as *const u16))
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))
            .unwrap_or_default()
    }
}

pub fn add(hwnd: HWND, tooltip: &str) {
    let mut nid = base_nid(hwnd);
    nid.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    nid.uCallbackMessage = super::WM_APP_TRAY;
    nid.hIcon = app_icon();
    set_tip(&mut nid, tooltip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

pub fn update_tooltip(hwnd: HWND, tooltip: &str) {
    let mut nid = base_nid(hwnd);
    nid.uFlags = NIF_TIP;
    set_tip(&mut nid, tooltip);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

pub fn remove(hwnd: HWND) {
    let nid = base_nid(hwnd);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

fn set_tip(nid: &mut NOTIFYICONDATAW, tooltip: &str) {
    let w = wide(tooltip);
    let n = w.len().min(nid.szTip.len());
    nid.szTip[..n].copy_from_slice(&w[..n]);
}

/// Show the right-click context menu at the cursor. Returns the chosen command
/// id (IDM_*), or 0 if dismissed.
pub fn show_context_menu(hwnd: HWND, connected: bool, split_on: bool) -> u32 {
    unsafe {
        let menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return 0,
        };

        let show = wide("Open");
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            super::IDM_SHOW as usize,
            PCWSTR(show.as_ptr()),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

        if connected {
            let d = wide("Disconnect");
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                super::IDM_DISCONNECT as usize,
                PCWSTR(d.as_ptr()),
            );
        } else {
            let c = wide("Connect");
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                super::IDM_CONNECT as usize,
                PCWSTR(c.as_ptr()),
            );
        }

        let split_label = if split_on {
            wide("Split tunneling: ON")
        } else {
            wide("Split tunneling: OFF")
        };
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            super::IDM_SPLIT as usize,
            PCWSTR(split_label.as_ptr()),
        );

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let e = wide("Exit");
        let _ = AppendMenuW(
            menu,
            MF_STRING,
            super::IDM_EXIT as usize,
            PCWSTR(e.as_ptr()),
        );

        // Required so the menu dismisses correctly on click-away.
        let _ = SetForegroundWindow(hwnd);

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);

        let cmd = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
        cmd.0 as u32
    }
}
