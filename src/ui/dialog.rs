//! Settings dialogs (MVP + Advanced), driven by the .rc templates in
//! manifest/app.rc via DialogBoxParamW. Both edit a working copy of AppConfig;
//! the caller's config is only overwritten when the MVP dialog returns OK.
#![cfg(windows)]

use core::mem::size_of;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::{AppConfig, Paths};
use crate::secret::Vault;
use crate::win::wide;
use crate::{import, shred, toml_writer};

/// Resource IDs -- MUST match manifest/resource.h.
mod ids {
    pub const IDD_SETTINGS: u16 = 101;
    pub const IDD_ADVANCED: u16 = 102;
    pub const IDD_PASSWORD: u16 = 103;

    pub const IDC_PWINFO: i32 = 3000;
    pub const IDE_PW1: i32 = 3001;
    pub const IDE_PW2: i32 = 3002;
    pub const IDL_PW2: i32 = 3004;

    pub const IDE_NAME: i32 = 1001;
    pub const IDE_HOSTNAME: i32 = 1002;
    pub const IDE_ADDRESSES: i32 = 1003;
    pub const IDE_USERNAME: i32 = 1004;
    pub const IDE_PASSWORD: i32 = 1005;
    pub const IDE_CERT: i32 = 1006;
    pub const IDE_COUNTRY: i32 = 1007;
    pub const IDE_REFRESH: i32 = 1008;
    pub const IDE_ENGINE: i32 = 1009;

    pub const IDC_PROTOCOL: i32 = 1101;
    pub const IDC_FALLBACK: i32 = 1102;
    pub const IDC_RIR: i32 = 1103;
    pub const IDC_LOGLEVEL: i32 = 1104;

    pub const IDC_SKIPVERIFY: i32 = 1201;
    pub const IDC_SPLIT: i32 = 1202;
    pub const IDC_KILLSWITCH: i32 = 1203;
    pub const IDC_IMPORT: i32 = 1208;
    pub const IDC_BROWSE: i32 = 1209;
    pub const IDC_ADVANCED: i32 = 1210;
    pub const IDC_CHANGEPW: i32 = 1211;

    pub const IDE_CLIENTRANDOM: i32 = 2001;
    pub const IDE_CUSTOMSNI: i32 = 2002;
    pub const IDE_MTU: i32 = 2003;
    pub const IDE_DNS: i32 = 2004;
    pub const IDE_KSPORTS: i32 = 2005;
    pub const IDE_SOCKS: i32 = 2006;
    pub const IDC_MODE: i32 = 2106;

    pub const IDC_IPV6: i32 = 2101;
    pub const IDC_ANTIDPI: i32 = 2102;
    pub const IDC_PQ: i32 = 2103;
    pub const IDC_CHANGEDNS: i32 = 2104;
}

const PROTOCOLS: &[&str] = &["http2", "http3"];
const FALLBACKS: &[&str] = &["(none)", "http2", "http3"];
const RIRS: &[&str] = &["ripencc", "arin", "apnic", "lacnic", "afrinic"];
const LOGLEVELS: &[&str] = &["error", "warn", "info", "debug", "trace"];
const MODES: &[&str] = &["tun", "socks"];

/// MAKEINTRESOURCE: a resource id encoded as a PCWSTR.
fn res(id: u16) -> PCWSTR {
    PCWSTR(id as usize as *const u16)
}

// The App's vault, made reachable to the (modal) settings dialog's
// change-password handler without threading it through every dialog function.
// Valid only for the duration of show_settings (modal, single-threaded).
thread_local! {
    static CHANGE_PW_VAULT: std::cell::Cell<*mut Vault> =
        std::cell::Cell::new(std::ptr::null_mut());
}

/// Show the settings dialog. Returns true and updates `cfg` if the user hit OK.
/// `vault` is the App's live vault, updated in place if the user changes the
/// password (that action persists immediately, independent of OK/Cancel).
pub fn show_settings(parent: HWND, cfg: &mut AppConfig, vault: &mut Vault) -> bool {
    let mut working = cfg.clone();
    CHANGE_PW_VAULT.with(|c| c.set(vault as *mut Vault));
    let ret = unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        DialogBoxParamW(
            hinst,
            res(ids::IDD_SETTINGS),
            parent,
            Some(settings_proc),
            LPARAM(&mut working as *mut AppConfig as isize),
        )
    };
    CHANGE_PW_VAULT.with(|c| c.set(std::ptr::null_mut()));
    if ret == 1 {
        *cfg = working;
        true
    } else {
        false
    }
}

fn show_advanced(parent: HWND, cfg: *mut AppConfig) {
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        DialogBoxParamW(
            hinst,
            res(ids::IDD_ADVANCED),
            parent,
            Some(advanced_proc),
            LPARAM(cfg as isize),
        );
    }
}

unsafe fn cfg_ptr(hdlg: HWND) -> *mut AppConfig {
    GetWindowLongPtrW(hdlg, GWLP_USERDATA) as *mut AppConfig
}

// --- dialog procedures ---

extern "system" fn settings_proc(hdlg: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    unsafe {
        match msg {
            WM_INITDIALOG => {
                SetWindowLongPtrW(hdlg, GWLP_USERDATA, lparam.0 as crate::win::WinLong);
                populate_settings(hdlg, &*(lparam.0 as *const AppConfig));
                1
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                match id {
                    ids::IDC_IMPORT => {
                        if let Some(path) =
                            open_file(hdlg, "Config files\0*.toml\0All files\0*.*\0\0")
                        {
                            if let Ok(text) = std::fs::read_to_string(&path) {
                                let cfg = &mut *cfg_ptr(hdlg);
                                // Read current edits first so import merges over them.
                                read_settings(hdlg, cfg);
                                match import::import_into(&text, cfg) {
                                    Ok(()) => populate_settings(hdlg, cfg),
                                    Err(e) => msgbox(hdlg, &e),
                                }
                            } else {
                                msgbox(hdlg, "could not read file");
                            }
                        }
                        1
                    }
                    ids::IDC_ADVANCED => {
                        // Persist current MVP edits before opening advanced, so
                        // an import/round-trip does not clobber them.
                        let cfg = cfg_ptr(hdlg);
                        read_settings(hdlg, &mut *cfg);
                        show_advanced(hdlg, cfg);
                        1
                    }
                    ids::IDC_BROWSE => {
                        if let Some(path) =
                            open_file(hdlg, "Executables\0*.exe\0All files\0*.*\0\0")
                        {
                            set_text(hdlg, ids::IDE_ENGINE, &path);
                        }
                        1
                    }
                    ids::IDC_CHANGEPW => {
                        change_password_flow(hdlg);
                        1
                    }
                    x if x == IDOK.0 => {
                        read_settings(hdlg, &mut *cfg_ptr(hdlg));
                        let _ = EndDialog(hdlg, 1);
                        1
                    }
                    x if x == IDCANCEL.0 => {
                        let _ = EndDialog(hdlg, 0);
                        1
                    }
                    _ => 0,
                }
            }
            _ => 0,
        }
    }
}

extern "system" fn advanced_proc(hdlg: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    unsafe {
        match msg {
            WM_INITDIALOG => {
                SetWindowLongPtrW(hdlg, GWLP_USERDATA, lparam.0 as crate::win::WinLong);
                populate_advanced(hdlg, &*(lparam.0 as *const AppConfig));
                1
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                match id {
                    x if x == IDOK.0 => {
                        read_advanced(hdlg, &mut *cfg_ptr(hdlg));
                        let _ = EndDialog(hdlg, 1);
                        1
                    }
                    x if x == IDCANCEL.0 => {
                        let _ = EndDialog(hdlg, 0);
                        1
                    }
                    _ => 0,
                }
            }
            _ => 0,
        }
    }
}

// --- populate / read ---

fn populate_settings(hdlg: HWND, cfg: &AppConfig) {
    let s = &cfg.server;
    set_text(hdlg, ids::IDE_NAME, &s.name);
    set_text(hdlg, ids::IDE_HOSTNAME, &s.hostname);
    set_lines(hdlg, ids::IDE_ADDRESSES, &s.addresses);
    set_text(hdlg, ids::IDE_USERNAME, &s.username);
    set_text(hdlg, ids::IDE_PASSWORD, &s.password);
    set_text(hdlg, ids::IDE_CERT, &s.certificate_pem);
    combo_fill(hdlg, ids::IDC_PROTOCOL, PROTOCOLS, &s.upstream_protocol);
    let fb = if s.upstream_fallback_protocol.is_empty() {
        "(none)"
    } else {
        &s.upstream_fallback_protocol
    };
    combo_fill(hdlg, ids::IDC_FALLBACK, FALLBACKS, fb);
    set_check(hdlg, ids::IDC_SKIPVERIFY, s.skip_verification);

    set_check(hdlg, ids::IDC_SPLIT, cfg.geoip.enabled);
    combo_fill(hdlg, ids::IDC_RIR, RIRS, &cfg.geoip.rir);
    set_text(hdlg, ids::IDE_COUNTRY, &cfg.geoip.country);
    set_text(hdlg, ids::IDE_REFRESH, &cfg.geoip.refresh_hours.to_string());

    set_check(hdlg, ids::IDC_KILLSWITCH, cfg.killswitch_enabled);
    combo_fill(hdlg, ids::IDC_LOGLEVEL, LOGLEVELS, &cfg.log_level);
    set_text(hdlg, ids::IDE_ENGINE, &cfg.engine_exe);
}

fn read_settings(hdlg: HWND, cfg: &mut AppConfig) {
    let s = &mut cfg.server;
    s.name = get_text(hdlg, ids::IDE_NAME);
    s.hostname = get_text(hdlg, ids::IDE_HOSTNAME).trim().to_string();
    s.addresses = get_lines(hdlg, ids::IDE_ADDRESSES);
    s.username = get_text(hdlg, ids::IDE_USERNAME);
    s.password = get_text(hdlg, ids::IDE_PASSWORD);
    s.certificate_pem = get_text(hdlg, ids::IDE_CERT).trim().to_string();
    s.upstream_protocol = combo_value(hdlg, ids::IDC_PROTOCOL, PROTOCOLS);
    let fb = combo_value(hdlg, ids::IDC_FALLBACK, FALLBACKS);
    s.upstream_fallback_protocol = if fb == "(none)" { String::new() } else { fb };
    s.skip_verification = get_check(hdlg, ids::IDC_SKIPVERIFY);

    cfg.geoip.enabled = get_check(hdlg, ids::IDC_SPLIT);
    cfg.geoip.rir = combo_value(hdlg, ids::IDC_RIR, RIRS);
    cfg.geoip.country = get_text(hdlg, ids::IDE_COUNTRY).trim().to_uppercase();
    if let Ok(h) = get_text(hdlg, ids::IDE_REFRESH).trim().parse::<u64>() {
        cfg.geoip.refresh_hours = h.max(1);
    }

    cfg.killswitch_enabled = get_check(hdlg, ids::IDC_KILLSWITCH);
    cfg.log_level = combo_value(hdlg, ids::IDC_LOGLEVEL, LOGLEVELS);
    cfg.engine_exe = get_text(hdlg, ids::IDE_ENGINE).trim().to_string();
}

fn populate_advanced(hdlg: HWND, cfg: &AppConfig) {
    let s = &cfg.server;
    combo_fill(hdlg, ids::IDC_MODE, MODES, &cfg.listener_mode);
    set_text(hdlg, ids::IDE_SOCKS, &cfg.socks_address);
    set_check(hdlg, ids::IDC_IPV6, s.has_ipv6);
    set_check(hdlg, ids::IDC_ANTIDPI, s.anti_dpi);
    set_check(hdlg, ids::IDC_PQ, cfg.post_quantum_enabled);
    set_check(hdlg, ids::IDC_CHANGEDNS, cfg.change_system_dns);
    set_text(hdlg, ids::IDE_CLIENTRANDOM, &s.client_random);
    set_text(hdlg, ids::IDE_CUSTOMSNI, &s.custom_sni);
    set_text(hdlg, ids::IDE_MTU, &cfg.mtu_size.to_string());
    set_lines(hdlg, ids::IDE_DNS, &s.dns_upstreams);
    let ports: Vec<String> = cfg
        .killswitch_allow_ports
        .iter()
        .map(|p| p.to_string())
        .collect();
    set_text(hdlg, ids::IDE_KSPORTS, &ports.join(", "));
}

fn read_advanced(hdlg: HWND, cfg: &mut AppConfig) {
    cfg.listener_mode = combo_value(hdlg, ids::IDC_MODE, MODES);
    let socks = get_text(hdlg, ids::IDE_SOCKS).trim().to_string();
    if !socks.is_empty() {
        cfg.socks_address = socks;
    }
    cfg.server.has_ipv6 = get_check(hdlg, ids::IDC_IPV6);
    cfg.server.anti_dpi = get_check(hdlg, ids::IDC_ANTIDPI);
    cfg.post_quantum_enabled = get_check(hdlg, ids::IDC_PQ);
    cfg.change_system_dns = get_check(hdlg, ids::IDC_CHANGEDNS);
    cfg.server.client_random = get_text(hdlg, ids::IDE_CLIENTRANDOM).trim().to_string();
    cfg.server.custom_sni = get_text(hdlg, ids::IDE_CUSTOMSNI).trim().to_string();
    if let Ok(m) = get_text(hdlg, ids::IDE_MTU).trim().parse::<u32>() {
        cfg.mtu_size = m;
    }
    cfg.server.dns_upstreams = get_lines(hdlg, ids::IDE_DNS);
    cfg.killswitch_allow_ports = get_text(hdlg, ids::IDE_KSPORTS)
        .split(',')
        .filter_map(|p| p.trim().parse::<u16>().ok())
        .collect();
}

// --- control helpers ---

fn set_text(hdlg: HWND, id: i32, text: &str) {
    unsafe {
        let w = wide(text);
        let _ = SetDlgItemTextW(hdlg, id, PCWSTR(w.as_ptr()));
    }
}

fn get_text(hdlg: HWND, id: i32) -> String {
    unsafe {
        let mut buf = [0u16; 8192];
        let n = GetDlgItemTextW(hdlg, id, &mut buf) as usize;
        String::from_utf16_lossy(&buf[..n])
    }
}

fn set_lines(hdlg: HWND, id: i32, items: &[String]) {
    set_text(hdlg, id, &items.join("\r\n"));
}

fn get_lines(hdlg: HWND, id: i32) -> Vec<String> {
    get_text(hdlg, id)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

fn set_check(hdlg: HWND, id: i32, checked: bool) {
    let state = if checked {
        BST_CHECKED.0
    } else {
        BST_UNCHECKED.0
    } as usize;
    unsafe {
        SendDlgItemMessageW(hdlg, id, BM_SETCHECK, WPARAM(state), LPARAM(0));
    }
}

fn get_check(hdlg: HWND, id: i32) -> bool {
    unsafe {
        SendDlgItemMessageW(hdlg, id, BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == BST_CHECKED.0 as isize
    }
}

fn combo_fill(hdlg: HWND, id: i32, options: &[&str], selected: &str) {
    unsafe {
        SendDlgItemMessageW(hdlg, id, CB_RESETCONTENT, WPARAM(0), LPARAM(0));
        let mut sel = 0usize;
        for (i, opt) in options.iter().enumerate() {
            let w = wide(opt);
            SendDlgItemMessageW(
                hdlg,
                id,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(w.as_ptr() as isize),
            );
            if *opt == selected {
                sel = i;
            }
        }
        SendDlgItemMessageW(hdlg, id, CB_SETCURSEL, WPARAM(sel), LPARAM(0));
    }
}

fn combo_value(hdlg: HWND, id: i32, options: &[&str]) -> String {
    unsafe {
        let sel = SendDlgItemMessageW(hdlg, id, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0;
        if sel >= 0 && (sel as usize) < options.len() {
            options[sel as usize].to_string()
        } else {
            options.first().copied().unwrap_or_default().to_string()
        }
    }
}

fn msgbox(hdlg: HWND, text: &str) {
    unsafe {
        let t = wide(text);
        let cap = wide("TrustTunnel");
        MessageBoxW(
            hdlg,
            PCWSTR(t.as_ptr()),
            PCWSTR(cap.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

/// Common File-Open dialog. `filter` is the Win32 double-NUL filter string.
fn open_file(parent: HWND, filter: &str) -> Option<String> {
    unsafe {
        let filter_w = wide(filter);
        let mut file_buf = [0u16; 1024];
        let mut ofn = OPENFILENAMEW {
            lStructSize: size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: parent,
            lpstrFilter: PCWSTR(filter_w.as_ptr()),
            lpstrFile: windows::core::PWSTR(file_buf.as_mut_ptr()),
            nMaxFile: file_buf.len() as u32,
            Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
            ..Default::default()
        };
        if GetOpenFileNameW(&mut ofn).as_bool() {
            let end = file_buf
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(file_buf.len());
            Some(String::from_utf16_lossy(&file_buf[..end]))
        } else {
            None
        }
    }
}

// --- password dialog + unlock flow ---

const MIN_PASSWORD_LEN: usize = 8;

struct PwCtx {
    create: bool,
    out: String,
}

/// Prompt for a password. `create` shows a confirm field and requires a match.
/// Returns None if the user cancels.
pub fn prompt_password(create: bool) -> Option<String> {
    let mut ctx = PwCtx {
        create,
        out: String::new(),
    };
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).unwrap_or_default().into();
        let ret = DialogBoxParamW(
            hinst,
            res(ids::IDD_PASSWORD),
            HWND::default(),
            Some(password_proc),
            LPARAM(&mut ctx as *mut PwCtx as isize),
        );
        if ret == 1 {
            Some(ctx.out)
        } else {
            None
        }
    }
}

unsafe fn pw_ctx(hdlg: HWND) -> *mut PwCtx {
    GetWindowLongPtrW(hdlg, GWLP_USERDATA) as *mut PwCtx
}

extern "system" fn password_proc(hdlg: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    unsafe {
        match msg {
            WM_INITDIALOG => {
                SetWindowLongPtrW(hdlg, GWLP_USERDATA, lparam.0 as crate::win::WinLong);
                let ctx = &*(lparam.0 as *const PwCtx);
                if ctx.create {
                    set_text(
                        hdlg,
                        ids::IDC_PWINFO,
                        "Create a password to protect your settings:",
                    );
                } else {
                    set_text(hdlg, ids::IDC_PWINFO, "Enter password:");
                    if let Ok(h) = GetDlgItem(hdlg, ids::IDE_PW2) {
                        let _ = ShowWindow(h, SW_HIDE);
                    }
                    if let Ok(h) = GetDlgItem(hdlg, ids::IDL_PW2) {
                        let _ = ShowWindow(h, SW_HIDE);
                    }
                }
                1
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                match id {
                    x if x == IDOK.0 => {
                        let ctx = &mut *pw_ctx(hdlg);
                        let pw1 = get_text(hdlg, ids::IDE_PW1);
                        if pw1.chars().count() < MIN_PASSWORD_LEN {
                            msgbox(hdlg, "Password must be at least 8 characters.");
                            return 1;
                        }
                        if ctx.create {
                            let pw2 = get_text(hdlg, ids::IDE_PW2);
                            if pw1 != pw2 {
                                msgbox(hdlg, "Passwords do not match.");
                                return 1;
                            }
                        }
                        ctx.out = pw1;
                        let _ = EndDialog(hdlg, 1);
                        1
                    }
                    x if x == IDCANCEL.0 => {
                        let _ = EndDialog(hdlg, 0);
                        1
                    }
                    _ => 0,
                }
            }
            _ => 0,
        }
    }
}

/// Change the settings password. Verifies the current password against the
/// on-disk vault, re-keys with a new one, and re-seals the PERSISTED config
/// (independent of any unsaved edits in the dialog). Updates the App's live
/// vault in place so later saves use the new password.
fn change_password_flow(hdlg: HWND) {
    let enc = Paths::settings_file();
    let blob = match std::fs::read(&enc) {
        Ok(b) => b,
        Err(_) => {
            msgbox(hdlg, "No settings file to change the password for.");
            return;
        }
    };

    let Some(old) = prompt_password(false) else {
        return;
    };
    let (_old_vault, plaintext) = match Vault::open(&blob, &old) {
        Ok(x) => x,
        Err(_) => {
            msgbox(hdlg, "Current password is incorrect.");
            return;
        }
    };

    let Some(new) = prompt_password(true) else {
        return;
    };
    let new_vault = match Vault::create(&new) {
        Ok(v) => v,
        Err(e) => {
            msgbox(hdlg, &e);
            return;
        }
    };
    let new_blob = match new_vault.seal(&plaintext) {
        Ok(b) => b,
        Err(e) => {
            msgbox(hdlg, &e);
            return;
        }
    };
    if toml_writer::write_atomic(&enc, &new_blob).is_err() {
        msgbox(hdlg, "Could not write settings file.");
        return;
    }

    // Point the App's live vault at the new key so subsequent saves match.
    let vptr = CHANGE_PW_VAULT.with(|c| c.get());
    if !vptr.is_null() {
        unsafe {
            *vptr = new_vault;
        }
    }
    msgbox(hdlg, "Password changed.");
}

/// Message box with no owner window (used before the main window exists).
fn info(text: &str) {
    unsafe {
        let t = wide(text);
        let c = wide("TrustTunnel");
        MessageBoxW(
            HWND::default(),
            PCWSTR(t.as_ptr()),
            PCWSTR(c.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}

/// Unlock the settings vault (or create it on first run). Returns None if the
/// user cancels -- the caller should then exit without starting.
pub fn unlock_or_create() -> Option<(Vault, AppConfig)> {
    let enc = Paths::settings_file();

    if enc.is_file() {
        let blob = std::fs::read(&enc).ok()?;
        loop {
            let pw = prompt_password(false)?; // None -> user cancelled -> exit
            match Vault::open(&blob, &pw) {
                Ok((vault, plaintext)) => {
                    // Self-check: the tag already proved the password is correct
                    // and the bytes are intact. Now confirm the plaintext really
                    // is a valid config. Do NOT fall back to defaults on failure
                    // -- that would overwrite the (good) file on the next save.
                    let text = match String::from_utf8(plaintext) {
                        Ok(t) => t,
                        Err(_) => {
                            info("Settings decrypted but are not valid text -- file is corrupt.");
                            return None;
                        }
                    };
                    match AppConfig::from_toml(&text) {
                        Ok(cfg) => return Some((vault, cfg)),
                        Err(e) => {
                            info(&format!(
                                "Settings decrypted but are invalid ({e}).\n\
                                 The file may be corrupt or from a newer version.\n\
                                 To start over, delete:\n{}",
                                Paths::settings_file().display()
                            ));
                            return None;
                        }
                    }
                }
                Err(e) => info(&e), // wrong password / corrupt -> retry
            }
        }
    }

    // First run. Migrate a legacy plaintext settings.toml if present.
    let mut cfg = AppConfig::default();
    let legacy = Paths::legacy_settings_file();
    let mut migrated = false;
    if let Ok(text) = std::fs::read_to_string(&legacy) {
        if let Ok(c) = AppConfig::from_toml(&text) {
            cfg = c;
            migrated = true;
        }
    }

    let pw = prompt_password(true)?; // None -> exit
    let vault = match Vault::create(&pw) {
        Ok(v) => v,
        Err(e) => {
            info(&e);
            return None;
        }
    };
    match vault.seal(cfg.to_toml().as_bytes()) {
        Ok(blob) => {
            if toml_writer::write_atomic(&enc, &blob).is_err() {
                info("Could not write settings file.");
                return None;
            }
        }
        Err(e) => {
            info(&e);
            return None;
        }
    }
    if migrated {
        shred::shred_file(&legacy);
    }
    Some((vault, cfg))
}
