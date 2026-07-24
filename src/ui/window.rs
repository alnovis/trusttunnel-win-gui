//! Main window: a connect/disconnect toggle, split-tunneling checkbox, a
//! refresh-geoip button, and a status line. Minimizes to the tray.
#![cfg(windows)]

use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, DrawTextW, FillRect, FrameRect, InvalidateRect, SetBkMode,
    SetTextColor, DT_CENTER, DT_SINGLELINE, DT_VCENTER, HGDIOBJ, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED, DRAWITEMSTRUCT, ODS_SELECTED};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use super::*;
use crate::app::App;
use crate::config::AppConfig;
use crate::secret::Vault;
use crate::win::wide;

const CLASS_NAME: &str = "TrustTunnelGuiWindow";
const WINDOW_TITLE: &str = concat!("TrustTunnel v", env!("CARGO_PKG_VERSION"));
const WATCHDOG_TIMER: usize = 1;

/// Bring an already-running instance's window to the foreground (it may be
/// hidden in the tray). Returns true if a window was found. Used by the
/// single-instance guard in main.
pub fn activate_existing() -> bool {
    unsafe {
        let class = wide(CLASS_NAME);
        match FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) {
            Ok(hwnd) if !hwnd.is_invalid() => {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
                true
            }
            _ => false,
        }
    }
}

/// Entry point: register the class, create the window, pump messages.
/// Called after the settings vault has been unlocked.
pub fn run(vault: Vault, cfg: AppConfig) {
    unsafe {
        let hinst: HINSTANCE = GetModuleHandleW(None).expect("module handle").into();

        let class = wide(CLASS_NAME);
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: hinst,
            lpszClassName: PCWSTR(class.as_ptr()),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            // Embedded app icon (resource id 1 in app.rc).
            hIcon: LoadIconW(hinst, PCWSTR(1usize as *const u16)).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&wc);

        // App lives for the whole message loop; hand ownership to the window
        // via lpParam and reclaim it in WM_NCDESTROY.
        let app = Box::new(App::new(vault, cfg));
        let app_ptr = Box::into_raw(app);

        let title = wide(WINDOW_TITLE);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            360,
            320,
            None,
            None,
            hinst,
            Some(app_ptr as *const _),
        )
        .expect("create window");

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Retrieve the App pointer stashed in the window's user data.
unsafe fn app_from(hwnd: HWND) -> Option<*mut App> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        None
    } else {
        Some(ptr as *mut App)
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_NCCREATE => {
                // Stash the App pointer passed via lpParam.
                let cs = lparam.0 as *const CREATESTRUCTW;
                if !cs.is_null() {
                    let app_ptr = (*cs).lpCreateParams as isize;
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_ptr as crate::win::WinLong);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CREATE => {
                create_controls(hwnd);
                tray::add(hwnd, "TrustTunnel: disconnected");
                // Watchdog tick every 2s. TODO: run the blocking external probe
                // on a worker thread and feed its result into tick() instead of
                // None (crash/liveness recovery works with None already).
                SetTimer(hwnd, WATCHDOG_TIMER, 2000, None);
                refresh_ui(hwnd);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == WATCHDOG_TIMER => {
                if let Some(p) = app_from(hwnd) {
                    // service_tick gates the background probe by state and folds
                    // its latest result into the watchdog decision.
                    (*p).service_tick();
                }
                refresh_ui(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as i32;
                handle_command(hwnd, id);
                LRESULT(0)
            }
            WM_DRAWITEM => {
                let dis = lparam.0 as *const DRAWITEMSTRUCT;
                if !dis.is_null() && (*dis).CtlID == IDC_TOGGLE as u32 {
                    draw_toggle(hwnd, &*dis);
                    return LRESULT(1); // handled
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            m if m == WM_APP_GEOIP_DONE => {
                if let Some(p) = app_from(hwnd) {
                    let msg = (*p).finish_geoip_refresh();
                    set_status(hwnd, &msg);
                }
                let _ = EnableWindow(control(hwnd, IDC_REFRESH), true);
                refresh_ui(hwnd);
                LRESULT(0)
            }
            m if m == WM_APP_TRAY => {
                // lParam low word carries the mouse event.
                let event = (lparam.0 & 0xFFFF) as u32;
                match event {
                    WM_LBUTTONDBLCLK => {
                        let _ = ShowWindow(hwnd, SW_SHOW);
                        let _ = SetForegroundWindow(hwnd);
                    }
                    WM_RBUTTONUP | WM_CONTEXTMENU => {
                        if let Some(p) = app_from(hwnd) {
                            let app = &mut *p;
                            let connected = app.is_connected();
                            let split = app.split_enabled();
                            let cmd = tray::show_context_menu(hwnd, connected, split);
                            dispatch_menu(hwnd, cmd);
                        }
                    }
                    _ => {}
                }
                LRESULT(0)
            }
            WM_SYSCOMMAND if (wparam.0 & 0xFFF0) == SC_MINIMIZE as usize => {
                // Minimize hides to tray instead of the taskbar.
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }
            WM_CLOSE => {
                // Close hides to tray; exit only via the tray menu.
                let _ = ShowWindow(hwnd, SW_HIDE);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                tray::remove(hwnd);
                // Reclaim and drop the App (stops the engine via Drop).
                if let Some(p) = app_from(hwnd) {
                    drop(Box::from_raw(p));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

unsafe fn create_controls(hwnd: HWND) {
    let hinst: HINSTANCE = GetModuleHandleW(None).map(Into::into).unwrap_or_default();

    let button = wide("BUTTON");
    let static_cls = wide("STATIC");

    // `move` so the closure copies hwnd/hinst by value (they are Copy);
    // otherwise it would borrow hinst and pass &HINSTANCE.
    let mk = move |text: &str, style: WINDOW_STYLE, x, y, w, h, id: i32, class: &[u16]| {
        let t = wide(text);
        let _ = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class.as_ptr()),
            PCWSTR(t.as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            w,
            h,
            hwnd,
            HMENU(id as usize as *mut _),
            hinst,
            None,
        );
    };

    // BS_* are plain i32 style bits -> wrap in WINDOW_STYLE.
    let pushbutton = WINDOW_STYLE(BS_PUSHBUTTON as u32);
    let checkbox = WINDOW_STYLE(BS_AUTOCHECKBOX as u32);

    // One owner-drawn toggle: we paint it ourselves in WM_DRAWITEM so the label
    // and colour follow the connection state (green = connected, amber = in
    // progress, neutral = off). The text is empty here -- draw_toggle supplies it.
    let ownerdraw = WINDOW_STYLE(BS_OWNERDRAW as u32);
    mk("", ownerdraw, 20, 20, 300, 40, IDC_TOGGLE, &button);
    mk(
        "Split tunneling (geoip)",
        checkbox,
        20,
        70,
        300,
        24,
        IDC_SPLIT,
        &button,
    );
    mk(
        "Refresh geoip list",
        pushbutton,
        20,
        104,
        300,
        30,
        IDC_REFRESH,
        &button,
    );
    mk(
        "Status: disconnected",
        WINDOW_STYLE(0),
        20,
        150,
        320,
        40,
        IDC_STATUS,
        &static_cls,
    );
    mk(
        "Settings...",
        pushbutton,
        20,
        196,
        300,
        30,
        IDC_SETTINGS,
        &button,
    );
    // Version, so it is obvious which build is running (helps when a stale
    // instance is still in the tray -- the new launch just surfaces the old one).
    mk(
        concat!("v", env!("CARGO_PKG_VERSION")),
        WINDOW_STYLE(0),
        20,
        238,
        320,
        12,
        -1,
        &static_cls,
    );
}

fn handle_command(hwnd: HWND, id: i32) {
    unsafe {
        let Some(p) = app_from(hwnd) else { return };
        let app = &mut *p;
        match id {
            IDC_TOGGLE => {
                // Active (connected or working towards it) -> stop; otherwise start.
                if toggle_is_active(app.state()) {
                    app.disconnect();
                } else if let Err(e) = app.connect() {
                    set_status(hwnd, &format!("Error: {e}"));
                }
                refresh_ui(hwnd);
            }
            IDC_SPLIT => {
                let checked = is_checked(hwnd, IDC_SPLIT);
                if let Err(e) = app.set_split_enabled(checked) {
                    set_status(hwnd, &format!("Error: {e}"));
                }
                refresh_ui(hwnd);
            }
            IDC_REFRESH => {
                match app.begin_geoip_refresh() {
                    Some(job) => {
                        set_status(hwnd, "Refreshing geoip...");
                        let _ = EnableWindow(control(hwnd, IDC_REFRESH), false);
                        // HWND is not Send; pass the raw pointer value across and
                        // rebuild it in the worker. PostMessageW is cross-thread safe.
                        let hwnd_raw = hwnd.0 as isize;
                        std::thread::spawn(move || {
                            let r = crate::geoip::refresh(&job.geoip).map(|v| v.len());
                            let _ = job.tx.send(r);
                            let h = HWND(hwnd_raw as *mut core::ffi::c_void);
                            let _ = PostMessageW(h, super::WM_APP_GEOIP_DONE, WPARAM(0), LPARAM(0));
                        });
                    }
                    None => set_status(hwnd, "Refresh already in progress"),
                }
            }
            IDC_SETTINGS => {
                if super::dialog::show_settings(hwnd, &mut app.cfg, &mut app.vault) {
                    if let Err(e) = app.apply_settings() {
                        set_status(hwnd, &format!("Error: {e}"));
                    }
                }
                refresh_ui(hwnd);
            }
            _ => {}
        }
    }
}

/// Dispatch a tray-menu command id (IDM_*).
fn dispatch_menu(hwnd: HWND, cmd: u32) {
    unsafe {
        let Some(p) = app_from(hwnd) else { return };
        let app = &mut *p;
        match cmd {
            IDM_SHOW => {
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
            IDM_CONNECT => {
                let _ = app.connect();
                refresh_ui(hwnd);
            }
            IDM_DISCONNECT => {
                app.disconnect();
                refresh_ui(hwnd);
            }
            IDM_SPLIT => {
                let now = !app.split_enabled();
                let _ = app.set_split_enabled(now);
                refresh_ui(hwnd);
            }
            IDM_EXIT => {
                let _ = DestroyWindow(hwnd);
                PostQuitMessage(0);
            }
            _ => {}
        }
    }
}

// --- small UI helpers ---

unsafe fn control(hwnd: HWND, id: i32) -> HWND {
    GetDlgItem(hwnd, id).unwrap_or_default()
}

fn set_status(hwnd: HWND, text: &str) {
    unsafe {
        let t = wide(text);
        let _ = SetWindowTextW(control(hwnd, IDC_STATUS), PCWSTR(t.as_ptr()));
    }
}

fn is_checked(hwnd: HWND, id: i32) -> bool {
    unsafe {
        SendMessageW(control(hwnd, id), BM_GETCHECK, WPARAM(0), LPARAM(0)).0
            == BST_CHECKED.0 as isize
    }
}

fn set_checked(hwnd: HWND, id: i32, checked: bool) {
    unsafe {
        let state = if checked {
            BST_CHECKED.0
        } else {
            BST_UNCHECKED.0
        } as usize;
        let _ = SendMessageW(control(hwnd, id), BM_SETCHECK, WPARAM(state), LPARAM(0));
    }
}

/// Reflect current App state into the controls + tray tooltip.
fn refresh_ui(hwnd: HWND) {
    use crate::engine_state::{ConnState, FailReason};
    unsafe {
        let Some(p) = app_from(hwnd) else { return };
        let app = &mut *p;
        set_checked(hwnd, IDC_SPLIT, app.split_enabled());

        let (short, long) = match app.state() {
            ConnState::Idle | ConnState::Disconnected => {
                ("disconnected", "Status: disconnected".to_string())
            }
            ConnState::Connecting => ("connecting", "Status: connecting...".to_string()),
            ConnState::Connected => ("connected", "Status: connected".to_string()),
            ConnState::Reconnecting => ("reconnecting", "Status: reconnecting...".to_string()),
            ConnState::Crashed => (
                "reconnecting",
                "Status: engine stopped, recovering...".to_string(),
            ),
            ConnState::Failed(reason) => {
                let why = match reason {
                    FailReason::Auth => "authentication failed",
                    FailReason::Certificate => "certificate error",
                    FailReason::Config => "bad configuration",
                    FailReason::Network => "network error",
                };
                let detail = app.last_error().unwrap_or(why);
                ("failed", format!("Status: FAILED -- {detail}"))
            }
        };
        set_status(hwnd, &long);
        tray::update_tooltip(
            hwnd,
            &format!("TrustTunnel v{}: {short}", env!("CARGO_PKG_VERSION")),
        );
        // Repaint the toggle so its label/colour track the new state.
        let _ = InvalidateRect(control(hwnd, IDC_TOGGLE), None, true);
    }
}

/// Is the tunnel connected or actively working towards it? Drives both the
/// toggle's click action (active -> disconnect) and its appearance.
fn toggle_is_active(state: &crate::engine_state::ConnState) -> bool {
    use crate::engine_state::ConnState::*;
    matches!(state, Connecting | Connected | Reconnecting | Crashed)
}

// COLORREF is 0x00BBGGRR. Keep the palette ASCII-simple and Win7-safe (plain GDI).
const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// Label + background + text colour for the toggle in a given state.
fn toggle_look(state: &crate::engine_state::ConnState) -> (&'static str, u32, u32) {
    use crate::engine_state::ConnState::*;
    const GREEN: u32 = rgb(46, 160, 67);
    const AMBER: u32 = rgb(214, 158, 46);
    const NEUTRAL: u32 = rgb(228, 228, 228);
    const WHITE: u32 = rgb(255, 255, 255);
    const BLACK: u32 = rgb(24, 24, 24);
    match state {
        Connected => ("Disconnect", GREEN, WHITE),
        Connecting => ("Connecting...", AMBER, BLACK),
        Reconnecting | Crashed => ("Reconnecting...", AMBER, BLACK),
        Idle | Disconnected | Failed(_) => ("Connect", NEUTRAL, BLACK),
    }
}

// Multiply each channel by ~0.85 -- used for the border and the pressed state.
fn darken(c: u32) -> u32 {
    let d = |ch: u32| (ch * 85 / 100) & 0xFF;
    rgb(
        d(c & 0xFF) as u8,
        d((c >> 8) & 0xFF) as u8,
        d((c >> 16) & 0xFF) as u8,
    )
}

/// Paint the owner-drawn toggle button (WM_DRAWITEM).
unsafe fn draw_toggle(hwnd: HWND, dis: &DRAWITEMSTRUCT) {
    let state = app_from(hwnd)
        .map(|p| (*p).state().clone())
        .unwrap_or(crate::engine_state::ConnState::Disconnected);
    let (label, mut bg, fg) = toggle_look(&state);
    // Pressed -> darken so the click reads as a press.
    if dis.itemState.0 & ODS_SELECTED.0 != 0 {
        bg = darken(bg);
    }
    let hdc = dis.hDC;
    let mut rc = dis.rcItem;

    let fill = CreateSolidBrush(COLORREF(bg));
    FillRect(hdc, &rc, fill);
    let _ = DeleteObject(HGDIOBJ(fill.0));

    let border = CreateSolidBrush(COLORREF(darken(bg)));
    FrameRect(hdc, &rc, border);
    let _ = DeleteObject(HGDIOBJ(border.0));

    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, COLORREF(fg));
    let mut text: Vec<u16> = label.encode_utf16().collect();
    DrawTextW(
        hdc,
        &mut text,
        &mut rc,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}
