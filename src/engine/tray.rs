//! Hidden window + notification-area icon + message loop. Everything engine-side
//! runs on this one thread, driven by a timer and the foreground-window hook.

use super::Engine;
use crate::icon::{make_hicon, IconState};
use crate::ipc::Mode;
use crate::logging::log;
use crate::sys::wide;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_NOSOUND, NIM_ADD,
    NIM_DELETE, NIM_MODIFY, NIN_BALLOONUSERCLICK, NOTIFYICONDATAW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_TRAY: u32 = WM_APP + 1;
const WM_WAKE: u32 = WM_APP + 2;
const WM_FOREGROUND: u32 = WM_APP + 3;
const TIMER_ID: usize = 1;
const HOTKEY_ID: i32 = 1;

const ID_OPEN: usize = 1;
const ID_AUTO: usize = 2;
const ID_OFF: usize = 3;
const ID_PURGE: usize = 4;
const ID_EXIT: usize = 5;
const ID_UPDATE: usize = 6;
const ID_ON_BASE: usize = 100;

static HWND_MAIN: AtomicUsize = AtomicUsize::new(0);
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

struct Tray {
    icons: [HICON; 3],
    shown: Option<(IconState, String)>,
}

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
    static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) };
}

fn with_engine<R>(f: impl FnOnce(&mut Engine) -> R) -> Option<R> {
    ENGINE.with(|e| e.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

/// Called from the IPC thread: nudge the message loop to process queued commands.
pub fn wake(shared: &super::Shared) {
    let h = shared.hwnd.load(Ordering::SeqCst);
    if h != 0 {
        unsafe {
            PostMessageW(h as HWND, WM_WAKE, 0, 0);
        }
    }
}

fn icon_index(s: IconState) -> usize {
    match s {
        IconState::Idle => 0,
        IconState::Boosting => 1,
        IconState::Paused => 2,
    }
}

fn copy_into(dst: &mut [u16], s: &str) {
    let w: Vec<u16> = s.encode_utf16().take(dst.len() - 1).collect();
    dst[..w.len()].copy_from_slice(&w);
    dst[w.len()] = 0;
}

unsafe fn nid(hwnd: HWND) -> NOTIFYICONDATAW {
    let mut n: NOTIFYICONDATAW = std::mem::zeroed();
    n.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = hwnd;
    n.uID = 1;
    n
}

unsafe fn add_icon(hwnd: HWND) {
    let icon = TRAY.with(|t| t.borrow().as_ref().map(|t| t.icons[0])).unwrap_or(std::ptr::null_mut());
    let mut n = nid(hwnd);
    n.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    n.uCallbackMessage = WM_TRAY;
    n.hIcon = icon;
    copy_into(&mut n.szTip, "Avix");
    Shell_NotifyIconW(NIM_ADD, &n);
}

unsafe fn refresh_tray(hwnd: HWND) {
    let Some((state, tip, balloon)) = with_engine(|e| (e.icon_state(), e.tooltip(), e.take_balloon())) else {
        return;
    };
    TRAY.with(|t| {
        let mut t = t.borrow_mut();
        let Some(t) = t.as_mut() else { return };
        let want = (state, tip.clone());
        if t.shown.as_ref() != Some(&want) {
            let mut n = nid(hwnd);
            n.uFlags = NIF_ICON | NIF_TIP;
            n.hIcon = t.icons[icon_index(state)];
            copy_into(&mut n.szTip, &tip);
            Shell_NotifyIconW(NIM_MODIFY, &n);
            t.shown = Some(want);
        }
    });
    if let Some((title, text)) = balloon {
        let mut n = nid(hwnd);
        n.uFlags = NIF_INFO;
        copy_into(&mut n.szInfoTitle, &title);
        copy_into(&mut n.szInfo, &text);
        n.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND;
        Shell_NotifyIconW(NIM_MODIFY, &n);
    }
}

unsafe fn append(menu: HMENU, flags: u32, id: usize, text: &str) {
    let w = wide(text);
    AppendMenuW(menu, flags, id, w.as_ptr());
}

fn checked(b: bool) -> u32 {
    if b {
        MF_CHECKED
    } else {
        MF_UNCHECKED
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let Some((mode, forced, profiles, status, update)) =
        with_engine(|e| (e.mode(), e.forced_profile(), e.profile_names(), e.status_line(), e.update_available()))
    else {
        return;
    };
    let menu = CreatePopupMenu();
    append(menu, MF_STRING | MF_GRAYED, 0, &format!("Avix - {status}"));
    if let Some(u) = &update {
        append(menu, MF_STRING, ID_UPDATE, &format!("Update available: Avix {} - download...", u.version));
    }
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    append(menu, MF_STRING, ID_OPEN, "Open dashboard");
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    append(menu, MF_STRING | checked(mode == Mode::Auto), ID_AUTO, "Auto-boost (recommended)");
    let sub = CreatePopupMenu();
    for (i, p) in profiles.iter().enumerate() {
        let on = mode == Mode::On && forced.as_deref().map_or(i == 0, |f| f.eq_ignore_ascii_case(p));
        append(sub, MF_STRING | checked(on), ID_ON_BASE + i, p);
    }
    let label = wide("Boost now\tCtrl+Alt+B");
    AppendMenuW(menu, MF_POPUP | checked(mode == Mode::On), sub as usize, label.as_ptr());
    append(menu, MF_STRING | checked(mode == Mode::Off), ID_OFF, "Pause Avix");
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    append(menu, MF_STRING, ID_PURGE, "Free standby memory now");
    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
    append(menu, MF_STRING, ID_EXIT, "Exit Avix (restores everything)");

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_NONOTIFY, pt.x, pt.y, 0, hwnd, std::ptr::null())
        as usize;
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(menu);

    match cmd {
        ID_OPEN => {
            with_engine(|e| e.open_ui());
        }
        ID_AUTO => {
            with_engine(|e| e.set_mode(Mode::Auto, None));
        }
        ID_OFF => {
            with_engine(|e| e.set_mode(Mode::Off, None));
        }
        ID_PURGE => {
            with_engine(|e| e.purge_now());
        }
        ID_EXIT => {
            DestroyWindow(hwnd);
            return;
        }
        ID_UPDATE => {
            if let Some(u) = &update {
                crate::sys::elevation::open_url(&u.url);
            }
        }
        c if c >= ID_ON_BASE && c < ID_ON_BASE + profiles.len() => {
            let name = profiles[c - ID_ON_BASE].clone();
            with_engine(|e| e.set_mode(Mode::On, Some(name)));
        }
        _ => {}
    }
    refresh_tray(hwnd);
}

/// (Re)apply the poll interval and hotkey from the current config.
unsafe fn apply_runtime(hwnd: HWND) {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::UnregisterHotKey;
    let Some((poll, hotkey)) = with_engine(|e| (e.poll_ms(), e.hotkey_enabled())) else { return };
    SetTimer(hwnd, TIMER_ID, poll as u32, None);
    UnregisterHotKey(hwnd, HOTKEY_ID);
    if hotkey && RegisterHotKey(hwnd, HOTKEY_ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, b'B' as u32) == 0 {
        log("Hotkey Ctrl+Alt+B is taken by another app");
    }
}

unsafe extern "system" fn win_event(_: HWINEVENTHOOK, _: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
    let h = HWND_MAIN.load(Ordering::SeqCst);
    if h != 0 {
        PostMessageW(h as HWND, WM_FOREGROUND, 0, 0);
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER if wp == TIMER_ID => {
            with_engine(|e| e.tick());
            refresh_tray(hwnd);
            0
        }
        WM_WAKE => {
            let exit = with_engine(|e| e.handle_commands()).unwrap_or(false);
            if exit {
                DestroyWindow(hwnd);
            } else {
                apply_runtime(hwnd);
                refresh_tray(hwnd);
            }
            0
        }
        WM_FOREGROUND => {
            with_engine(|e| e.on_foreground());
            refresh_tray(hwnd);
            0
        }
        WM_HOTKEY => {
            with_engine(|e| e.toggle_boost());
            refresh_tray(hwnd);
            0
        }
        WM_TRAY => {
            match (lp as u32) & 0xFFFF {
                WM_LBUTTONUP | WM_LBUTTONDBLCLK => {
                    with_engine(|e| e.open_ui());
                }
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                // clicking a balloon: the update one opens the download page, others the dashboard
                NIN_BALLOONUSERCLICK => match with_engine(|e| e.update_available()).flatten() {
                    Some(u) => crate::sys::elevation::open_url(&u.url),
                    None => {
                        with_engine(|e| e.open_ui());
                    }
                },
                _ => {}
            }
            0
        }
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION => {
            if wp != 0 {
                with_engine(|e| e.shutdown());
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::SeqCst) => {
            // Explorer restarted: re-add the icon.
            TRAY.with(|t| {
                if let Some(t) = t.borrow_mut().as_mut() {
                    t.shown = None;
                }
            });
            add_icon(hwnd);
            refresh_tray(hwnd);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn run(engine: Engine, open_ui: bool) {
    unsafe {
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("AvixEngineWnd");
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wndproc);
        wc.hInstance = hinst;
        wc.lpszClassName = class.as_ptr();
        RegisterClassW(&wc);
        let title = wide("Avix Engine");
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            log("Failed to create engine window");
            return;
        }
        HWND_MAIN.store(hwnd as usize, Ordering::SeqCst);
        engine.shared.hwnd.store(hwnd as usize, Ordering::SeqCst);
        let tb = wide("TaskbarCreated");
        TASKBAR_CREATED.store(RegisterWindowMessageW(tb.as_ptr()), Ordering::SeqCst);

        ENGINE.with(|e| *e.borrow_mut() = Some(engine));

        let size = GetSystemMetrics(SM_CXSMICON).max(16) as u32;
        let icons = [
            make_hicon(size, IconState::Idle),
            make_hicon(size, IconState::Boosting),
            make_hicon(size, IconState::Paused),
        ];
        TRAY.with(|t| *t.borrow_mut() = Some(Tray { icons, shown: None }));
        add_icon(hwnd);

        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(win_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        apply_runtime(hwnd);
        with_engine(|e| e.tick());
        refresh_tray(hwnd);
        if open_ui {
            with_engine(|e| e.open_ui());
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if !hook.is_null() {
            UnhookWinEvent(hook);
        }
        let n = nid(hwnd);
        Shell_NotifyIconW(NIM_DELETE, &n);
        with_engine(|e| e.shutdown());
        TRAY.with(|t| {
            if let Some(t) = t.borrow_mut().take() {
                for i in t.icons {
                    DestroyIcon(i);
                }
            }
        });
    }
}
