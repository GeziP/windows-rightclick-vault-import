pub mod autostart;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use tracing::{error, info};

use crate::i18n::tr;

const TRAY_CALLBACK_MSG: u32 = 0x0400 + 1; // WM_USER + 1
const ID_MENU_SETTINGS: usize = 1001;
const ID_MENU_AUTOSTART: usize = 1002;
const ID_MENU_QUIT: usize = 1003;

struct TrayState {
    tooltip: String,
    exe_path: PathBuf,
    console_exe: PathBuf,
    autostart_enabled: bool,
    lang: String,
}

static SHUTDOWN_FLAG: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();
static TRAY_STATE: std::sync::OnceLock<Arc<Mutex<TrayState>>> = std::sync::OnceLock::new();
static WM_TASKBAR: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

#[cfg(windows)]
pub fn run_tray(app_data_dir: PathBuf) -> Result<()> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIM_ADD, NIM_DELETE};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DispatchMessageW, GetMessageW, RegisterClassExW,
        RegisterWindowMessageW, TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW,
        WS_OVERLAPPED,
    };
    use windows::core::w;

    let config = crate::config::AppConfig::load_or_init_in(app_data_dir.clone())
        .context("failed to load config for tray")?;
    let lang = config.language().to_string();
    let watch_count = config.watch.len();
    let exe_path = std::env::current_exe().context("failed to get current exe path")?;
    let console_exe = exe_path.with_file_name("kbintake.exe");

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let tooltip = if watch_count > 0 {
        tr("tray.tooltip_active", &lang)
            .replace("{}", &watch_count.to_string())
    } else {
        tr("tray.tooltip_idle", &lang).to_string()
    };

    let state = Arc::new(Mutex::new(TrayState {
        tooltip,
        exe_path: exe_path.clone(),
        console_exe,
        autostart_enabled: autostart::is_autostart_enabled().unwrap_or(false),
        lang,
    }));

    let wm_taskbarcreated = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };

    SHUTDOWN_FLAG.get_or_init(|| shutdown_flag.clone());
    TRAY_STATE.get_or_init(|| state.clone());
    WM_TASKBAR.get_or_init(|| wm_taskbarcreated);

    // Register window class
    let class_name = w!("KBIntakeTrayClass");
    let hmodule = unsafe { GetModuleHandleW(None) }.context("GetModuleHandleW failed")?;
    let hinstance = windows::Win32::Foundation::HINSTANCE(hmodule.0);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance,
        lpszClassName: class_name,
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };

    // Create hidden window
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            w!("KBIntake"),
            WINDOW_STYLE(WS_OVERLAPPED.0),
            0,
            0,
            0,
            0,
            HWND::default(),
            None,
            hinstance,
            None,
        )
    }
    .context("CreateWindowExW failed")?;

    // Load icon and add tray icon
    let hicon = load_tray_icon(&exe_path);
    let tip = get_state(|s| s.tooltip.clone());
    let nid = build_nid(hwnd, hicon, &tip);

    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }

    // Start watcher thread if watch configs exist
    if watch_count > 0 {
        let flag = shutdown_flag.clone();
        let dir = app_data_dir;
        std::thread::spawn(move || {
            let app = match crate::app::App::bootstrap_in(dir) {
                Ok(app) => app,
                Err(e) => {
                    error!("tray watcher bootstrap failed: {e:#}");
                    return;
                }
            };
            if let Err(e) = crate::agent::watcher::run_watcher(&app, None, flag) {
                error!("tray watcher exited with error: {e:#}");
            }
        });
    }

    info!("KBIntake tray started ({} watch dir(s))", watch_count);

    // Message loop
    let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
        if !ret.as_bool() {
            break;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }

    info!("KBIntake tray exiting");
    Ok(())
}

#[cfg(not(windows))]
pub fn run_tray(_app_data_dir: PathBuf) -> Result<()> {
    anyhow::bail!("tray mode is only supported on Windows")
}

fn get_state<F, R>(f: F) -> R
where
    F: FnOnce(&TrayState) -> R,
{
    let state = TRAY_STATE.get().expect("tray state not initialized");
    let s = state.lock().unwrap();
    f(&s)
}

fn update_state<F>(f: F)
where
    F: FnOnce(&mut TrayState),
{
    if let Some(state) = TRAY_STATE.get() {
        let mut s = state.lock().unwrap();
        f(&mut s);
    }
}

#[cfg(windows)]
fn build_nid(
    hwnd: windows::Win32::Foundation::HWND,
    hicon: windows::Win32::UI::WindowsAndMessaging::HICON,
    tooltip: &str,
) -> windows::Win32::UI::Shell::NOTIFYICONDATAW {
    use windows::Win32::UI::Shell::{NIF_ICON, NIF_MESSAGE, NIF_TIP, NOTIFYICONDATAW};

    let tooltip_wide: Vec<u16> = tooltip.encode_utf16().chain(std::iter::once(0)).collect();
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: TRAY_CALLBACK_MSG,
        hIcon: hicon,
        ..Default::default()
    };
    let tip_len = tooltip_wide.len().min(nid.szTip.len());
    nid.szTip[..tip_len].copy_from_slice(&tooltip_wide[..tip_len]);
    nid
}

#[cfg(windows)]
unsafe extern "system" fn wnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, PostMessageW, PostQuitMessage, WM_CLOSE, WM_COMMAND, WM_DESTROY,
        WM_LBUTTONDBLCLK, WM_RBUTTONUP,
    };

    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match msg {
        WM_DESTROY => {
            info!("tray window destroyed");
            unsafe { PostQuitMessage(0) };
        }
        x if x == TRAY_CALLBACK_MSG => match lparam.0 as u32 {
            WM_RBUTTONUP => show_context_menu(hwnd),
            WM_LBUTTONDBLCLK => open_settings(),
            _ => {}
        },
        WM_COMMAND => {
            let menu_id = wparam.0 & 0xFFFF;
            info!("menu command: id={menu_id}");
            match menu_id {
                ID_MENU_SETTINGS => open_settings(),
                ID_MENU_AUTOSTART => toggle_autostart(),
                ID_MENU_QUIT => {
                    if let Some(flag) = SHUTDOWN_FLAG.get() {
                        flag.store(true, Ordering::SeqCst);
                    }
                    unsafe {
                        let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                }
                _ => {}
            }
        }
        _ => {
            if let Some(&tb_msg) = WM_TASKBAR.get() {
                if msg == tb_msg {
                    readd_tray_icon(hwnd);
                }
            }
        }
    }));

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(windows)]
fn show_context_menu(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Foundation::{LPARAM, POINT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreatePopupMenu, DestroyMenu, GetCursorPos, InsertMenuW, MF_BYPOSITION, MF_CHECKED,
        MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow, TrackPopupMenu,
        TPM_BOTTOMALIGN, TPM_LEFTALIGN, WM_NULL,
    };
    use windows::core::{HSTRING, PCWSTR};

    let (lang, autostart_on) = get_state(|s| (s.lang.clone(), s.autostart_enabled));

    let menu = match unsafe { CreatePopupMenu() } {
        Ok(m) => m,
        Err(e) => {
            error!("CreatePopupMenu failed: {e:#}");
            return;
        }
    };

    let settings_text = HSTRING::from(tr("tray.menu_settings", &lang));
    let autostart_text = HSTRING::from(if autostart_on {
        tr("tray.menu_autostart_on", &lang)
    } else {
        tr("tray.menu_autostart_off", &lang)
    });
    let quit_text = HSTRING::from(tr("tray.menu_quit", &lang));

    unsafe {
        let _ = InsertMenuW(
            menu,
            0,
            MF_BYPOSITION | MF_STRING,
            ID_MENU_SETTINGS,
            PCWSTR(settings_text.as_ptr()),
        );
        let auto_flags = if autostart_on {
            MF_BYPOSITION | MF_STRING | MF_CHECKED
        } else {
            MF_BYPOSITION | MF_STRING
        };
        let _ = InsertMenuW(
            menu,
            1,
            auto_flags,
            ID_MENU_AUTOSTART,
            PCWSTR(autostart_text.as_ptr()),
        );
        let _ = InsertMenuW(
            menu,
            2,
            MF_BYPOSITION | MF_SEPARATOR,
            0,
            PCWSTR::null(),
        );
        let _ = InsertMenuW(
            menu,
            3,
            MF_BYPOSITION | MF_STRING,
            ID_MENU_QUIT,
            PCWSTR(quit_text.as_ptr()),
        );

        let mut pt = POINT::default();
        let _ = GetCursorPos(&mut pt);
        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );
        let _ = PostMessageW(hwnd, WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
    }
}

#[cfg(windows)]
fn open_settings() {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, PCWSTR};

    let console_exe = get_state(|s| s.console_exe.clone());
    std::thread::spawn(move || {
        let verb = HSTRING::from("open");
        let file = HSTRING::from(console_exe.display().to_string());
        let params = HSTRING::from("tui");
        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(file.as_ptr()),
                PCWSTR(params.as_ptr()),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            error!("ShellExecuteW failed for settings TUI");
        } else {
            info!("opened settings TUI: {}", console_exe.display());
        }
    });
}

#[cfg(windows)]
fn toggle_autostart() {
    let (exe_path, autostart_on) = get_state(|s| (s.exe_path.clone(), s.autostart_enabled));
    if autostart_on {
        if let Err(e) = autostart::remove_autostart() {
            error!("failed to remove autostart: {e:#}");
        }
    } else if let Err(e) = autostart::set_autostart(&exe_path) {
        error!("failed to set autostart: {e:#}");
    }
    update_state(|s| {
        s.autostart_enabled = autostart::is_autostart_enabled().unwrap_or(false);
    });
}

#[cfg(windows)]
fn readd_tray_icon(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::Shell::{Shell_NotifyIconW, NIM_ADD};

    let exe_path = get_state(|s| s.exe_path.clone());
    let tooltip = get_state(|s| s.tooltip.clone());
    let hicon = load_tray_icon(&exe_path);
    let nid = build_nid(hwnd, hicon, &tooltip);

    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

#[cfg(windows)]
fn load_tray_icon(exe_path: &std::path::Path) -> windows::Win32::UI::WindowsAndMessaging::HICON {
    use windows::Win32::Foundation::HINSTANCE;
    use windows::Win32::UI::WindowsAndMessaging::{LoadImageW, LR_DEFAULTSIZE, LR_LOADFROMFILE, IMAGE_ICON};
    use windows::core::HSTRING;

    let icon_path = exe_path.with_file_name("kbintake.ico");
    if icon_path.exists() {
        let icon_str = HSTRING::from(icon_path.display().to_string());
        unsafe {
            LoadImageW(
                HINSTANCE(std::ptr::null_mut()),
                &icon_str,
                IMAGE_ICON,
                16,
                16,
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            )
        }
        .ok()
        .map(|h| windows::Win32::UI::WindowsAndMessaging::HICON(h.0))
    } else {
        None
    }
    .unwrap_or_default()
}
