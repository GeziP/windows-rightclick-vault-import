//! `IExplorerCommand` implementation — static command that queues the selected item(s).
//!
//! Manual COM vtable approach since windows 0.58 doesn't expose `#[implement]`
//! for IExplorerCommand in a usable way.

use std::ffi::c_void;
use windows::core::{GUID, HRESULT};
use windows::core::Interface;
use windows::Win32::System::Com::CoTaskMemAlloc;
use windows::Win32::UI::Shell::{IShellItemArray, SIGDN_FILESYSPATH, ECS_ENABLED};

// COM HRESULT constants (u32 cast to i32 to avoid literal overflow).
const E_INVALIDARG: HRESULT = HRESULT(0x80070057u32 as i32);
const E_NOINTERFACE: HRESULT = HRESULT(0x80004002u32 as i32);
const E_NOTIMPL: HRESULT = HRESULT(0x80004001u32 as i32);
const E_OUTOFMEMORY: HRESULT = HRESULT(0x8007000Eu32 as i32);
const S_OK: HRESULT = HRESULT(0);

/// CLSID for the KBIntake Explorer command.
pub const CLSID_KBINTAKE_COMMAND: GUID = GUID::from_u128(0xA1B2_C3D4_E5F6_7890_ABCD_EF12_3456_7890);

// VTable for IExplorerCommand.
#[repr(C)]
struct ExplorerCommandVtbl {
    // IUnknown
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    // IExplorerCommand
    get_title: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut u16) -> HRESULT,
    get_icon: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut u16) -> HRESULT,
    get_tooltip: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut *mut u16) -> HRESULT,
    get_canonical_name: unsafe extern "system" fn(*mut c_void, *mut GUID) -> HRESULT,
    get_state: unsafe extern "system" fn(*mut c_void, *mut c_void, i32, *mut u32) -> HRESULT,
    invoke: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> HRESULT,
    get_flags: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    enum_sub_commands: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

/// The COM object.
#[repr(C)]
pub struct ExplorerCommandHandler {
    vtable: *const ExplorerCommandVtbl,
    ref_count: std::sync::atomic::AtomicI32,
}

static VTBL: ExplorerCommandVtbl = ExplorerCommandVtbl {
    query_interface: cmd_query_interface,
    add_ref: cmd_add_ref,
    release: cmd_release,
    get_title: cmd_get_title,
    get_icon: cmd_get_icon,
    get_tooltip: cmd_get_tooltip,
    get_canonical_name: cmd_get_canonical_name,
    get_state: cmd_get_state,
    invoke: cmd_invoke,
    get_flags: cmd_get_flags,
    enum_sub_commands: cmd_enum_sub_commands,
};

// GUIDs for interface IDs.
const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_C000_000000000046);
// IExplorerCommand IID: {a08ce4d0-fa25-44ab-b57c-c7b1c323e0b9}
const IID_IEXPLORER_COMMAND: GUID = GUID::from_u128(0xa08ce4d0_fa25_44ab_b57c_c7b1c323e0b9);

unsafe extern "system" fn cmd_query_interface(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    if riid.is_null() || ppv.is_null() {
        return E_INVALIDARG;
    }
    let riid = &*riid;
    if *riid == IID_IUNKNOWN || *riid == IID_IEXPLORER_COMMAND {
        *ppv = this;
        cmd_add_ref(this);
        S_OK
    } else {
        *ppv = std::ptr::null_mut();
        E_NOINTERFACE
    }
}

unsafe extern "system" fn cmd_add_ref(this: *mut c_void) -> u32 {
    let handler = &mut *(this as *mut ExplorerCommandHandler);
    handler.ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn cmd_release(this: *mut c_void) -> u32 {
    let handler = &mut *(this as *mut ExplorerCommandHandler);
    let new_count = handler.ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) as u32 - 1;
    if new_count == 0 {
        drop(Box::from_raw(this as *mut ExplorerCommandHandler));
    }
    new_count
}

unsafe extern "system" fn cmd_get_title(_this: *mut c_void, _psi: *mut c_void, ppsz_name: *mut *mut u16) -> HRESULT {
    if ppsz_name.is_null() {
        return E_INVALIDARG;
    }
    const TITLE: &[u16] = &[
        b'A' as u16, b'd' as u16, b'd' as u16, b' ' as u16,
        b't' as u16, b'o' as u16, b' ' as u16,
        b'K' as u16, b'n' as u16, b'o' as u16, b'w' as u16,
        b'l' as u16, b'e' as u16, b'd' as u16, b'g' as u16, b'e' as u16,
        b' ' as u16, b'B' as u16, b'a' as u16, b's' as u16, b'e' as u16,
        0,
    ];
    let ptr = unsafe { CoTaskMemAlloc(TITLE.len() * 2) } as *mut u16;
    if ptr.is_null() {
        return E_OUTOFMEMORY;
    }
    std::ptr::copy_nonoverlapping(TITLE.as_ptr(), ptr, TITLE.len());
    *ppsz_name = ptr;
    S_OK
}

unsafe extern "system" fn cmd_get_icon(_this: *mut c_void, _psi: *mut c_void, ppsz_icon: *mut *mut u16) -> HRESULT {
    if ppsz_icon.is_null() {
        return E_INVALIDARG;
    }
    *ppsz_icon = std::ptr::null_mut();
    S_OK
}

unsafe extern "system" fn cmd_get_tooltip(_this: *mut c_void, _psi: *mut c_void, ppsz_tip: *mut *mut u16) -> HRESULT {
    if ppsz_tip.is_null() {
        return E_INVALIDARG;
    }
    const TIP: &[u16] = &[
        b'I' as u16, b'm' as u16, b'p' as u16, b'o' as u16, b'r' as u16, b't' as u16,
        b' ' as u16, b'f' as u16, b'i' as u16, b'l' as u16, b'e' as u16,
        b'(' as u16, b's' as u16, b')' as u16, b' ' as u16,
        b'i' as u16, b'n' as u16, b't' as u16, b'o' as u16,
        b' ' as u16, b'K' as u16, b'B' as u16, b'I' as u16, b'n' as u16,
        b't' as u16, b'a' as u16, b'k' as u16, b'e' as u16,
        b' ' as u16, b'v' as u16, b'a' as u16, b'u' as u16, b'l' as u16, b't' as u16,
        0,
    ];
    let ptr = unsafe { CoTaskMemAlloc(TIP.len() * 2) } as *mut u16;
    if ptr.is_null() {
        return E_OUTOFMEMORY;
    }
    std::ptr::copy_nonoverlapping(TIP.as_ptr(), ptr, TIP.len());
    *ppsz_tip = ptr;
    S_OK
}

unsafe extern "system" fn cmd_get_canonical_name(_this: *mut c_void, pguid: *mut GUID) -> HRESULT {
    if pguid.is_null() {
        return E_INVALIDARG;
    }
    *pguid = CLSID_KBINTAKE_COMMAND;
    S_OK
}

unsafe extern "system" fn cmd_get_state(_this: *mut c_void, _psi: *mut c_void, _foktobeslow: i32, pstate: *mut u32) -> HRESULT {
    if pstate.is_null() {
        return E_INVALIDARG;
    }
    *pstate = ECS_ENABLED.0 as u32;
    S_OK
}

unsafe extern "system" fn cmd_get_flags(_this: *mut c_void, pflags: *mut u32) -> HRESULT {
    if pflags.is_null() {
        return E_INVALIDARG;
    }
    *pflags = 0; // ECF_DEFAULT
    S_OK
}

unsafe extern "system" fn cmd_enum_sub_commands(_this: *mut c_void, _ppenum: *mut *mut c_void) -> HRESULT {
    E_NOTIMPL
}

unsafe extern "system" fn cmd_invoke(_this: *mut c_void, psi: *mut c_void, _pbc: *mut c_void) -> HRESULT {
    let _ = _this; // unused

    if psi.is_null() {
        return E_INVALIDARG;
    }

    // Get IShellItemArray from raw pointer.
    let shell_items = IShellItemArray::from_raw_borrowed(&psi);
    let Some(shell_items) = shell_items else {
        return E_INVALIDARG;
    };

    let count = match shell_items.GetCount() {
        Ok(c) => c,
        Err(_) => return S_OK,
    };
    if count == 0 {
        return S_OK;
    }

    // Collect file paths from shell items.
    let mut paths = Vec::with_capacity(count as usize);
    for i in 0..count {
        if let Ok(item) = shell_items.GetItemAt(i) {
            if let Ok(path_bstr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                if let Ok(s) = path_bstr.to_string() {
                    paths.push(s);
                }
            }
        }
    }

    if paths.is_empty() {
        return S_OK;
    }

    // Spawn the KBIntake import process in the background.
    spawn_kbintake_import(&paths);

    S_OK
}

/// Create a new ExplorerCommandHandler boxed COM object.
pub fn create_handler() -> *mut ExplorerCommandHandler {
    Box::into_raw(Box::new(ExplorerCommandHandler {
        vtable: &VTBL,
        ref_count: std::sync::atomic::AtomicI32::new(1),
    }))
}

/// Spawn `kbintake.exe import --process <paths...>` in a detached process.
fn spawn_kbintake_import(paths: &[String]) {
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    let exe_path = find_kbintake_exe();
    let Some(exe_path) = exe_path else {
        return;
    };

    let mut cmd = Command::new(&exe_path);
    cmd.arg("import");
    cmd.arg("--process");
    for path in paths {
        cmd.arg(path);
    }
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let _ = cmd.spawn();
}

/// Locate `kbintake.exe`.
fn find_kbintake_exe() -> Option<String> {
    if let Ok(dll_path) = std::env::current_exe() {
        if let Some(parent) = dll_path.parent() {
            let candidate = parent.join("kbintake.exe");
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let candidate = format!(r"{}\Programs\kbintake\kbintake.exe", local_app_data);
        if std::path::Path::new(&candidate).exists() {
            return Some(candidate);
        }
    }

    if let Ok(path_env) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_env) {
            let candidate = dir.join("kbintake.exe");
            if candidate.exists() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    None
}
