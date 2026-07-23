//! Process discovery/verification and single-instance guard. Windows-only.
#![cfg(windows)]

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE, MAX_PATH,
};
use windows::Win32::System::Threading::{
    CreateMutexW, GetExitCodeProcess, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};

use crate::win::wide;

const STILL_ACTIVE: u32 = 259;

/// True if `pid` refers to a live process.
pub fn pid_alive(pid: u32) -> bool {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code: u32 = 0;
        let alive = GetExitCodeProcess(h, &mut code).is_ok() && code == STILL_ACTIVE;
        let _ = CloseHandle(h);
        alive
    }
}

/// True if `pid` is alive AND its image basename matches `exe_name`
/// (case-insensitive). Guards against PID reuse handing us a stranger.
pub fn pid_alive_and_named(pid: u32, exe_name: &str) -> bool {
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };

        let mut code: u32 = 0;
        if !(GetExitCodeProcess(h, &mut code).is_ok() && code == STILL_ACTIVE) {
            let _ = CloseHandle(h);
            return false;
        }

        let mut buf = [0u16; MAX_PATH as usize];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(
            h,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(buf.as_mut_ptr()),
            &mut size,
        )
        .is_ok();
        let _ = CloseHandle(h);
        if !ok {
            return false;
        }

        let path = String::from_utf16_lossy(&buf[..size as usize]);
        let base = path.rsplit(['\\', '/']).next().unwrap_or(&path);
        base.eq_ignore_ascii_case(exe_name)
    }
}

/// Best-effort terminate by PID.
pub fn terminate(pid: u32) {
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_TERMINATE, false, pid) {
            let _ = TerminateProcess(h, 1);
            let _ = CloseHandle(h);
        }
    }
}

/// Held for the life of the process to keep the named mutex owned.
pub struct SingleInstance {
    _handle: HANDLE,
}

/// Acquire a machine-global single-instance lock. Returns None if another
/// wrapper instance already holds it.
pub fn acquire_single_instance(name: &str) -> Option<SingleInstance> {
    unsafe {
        let n = wide(name);
        let handle = CreateMutexW(None, true, PCWSTR(n.as_ptr())).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }
        Some(SingleInstance { _handle: handle })
    }
}
