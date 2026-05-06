//! COM server exports: `DllGetClassObject`, `DllCanUnloadNow`, lock counting.

use std::ffi::c_void;
use std::sync::atomic::AtomicU32;

use windows::core::{GUID, HRESULT};

use crate::command::CLSID_KBINTAKE_COMMAND;
use crate::factory::create_factory;

// COM HRESULT constants.
const E_INVALIDARG: HRESULT = HRESULT(0x80070057u32 as i32);
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = HRESULT(0x80040111u32 as i32);

/// Global object + lock counter. COM can unload the DLL when both are zero.
static OBJECT_COUNT: AtomicU32 = AtomicU32::new(0);
static LOCK_COUNT: AtomicU32 = AtomicU32::new(0);

pub mod lock_count {
    use super::{LOCK_COUNT, OBJECT_COUNT};
    use std::sync::atomic::Ordering;

    pub fn increment() {
        LOCK_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    pub fn decrement() {
        LOCK_COUNT.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn object_addref() {
        OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
    }

    pub fn object_release() {
        OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn can_unload() -> bool {
        OBJECT_COUNT.load(Ordering::SeqCst) == 0 && LOCK_COUNT.load(Ordering::SeqCst) == 0
    }
}

const S_OK: HRESULT = HRESULT(0);
const S_FALSE: HRESULT = HRESULT(1);

/// Standard COM entry point. Returns the class factory for `rclsid`.
///
/// # Safety
/// `rclsid`, `riid`, and `ppv` must be valid non-null pointers. `ppv` must be
/// writable and the caller must not read from it on failure.
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_INVALIDARG;
    }

    let rclsid = &*rclsid;
    let _riid = &*riid; // Reserved for future IID checks.

    if *rclsid != CLSID_KBINTAKE_COMMAND {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let factory_ptr = create_factory();
    *ppv = factory_ptr as *mut c_void;
    S_OK
}

/// Standard COM entry point. Returns `S_OK` if the DLL can be safely unloaded.
///
/// # Safety
/// This function is `extern "system"` and `unsafe` only to satisfy COM's DLL
/// lifecycle contract. It does not dereference raw pointers.
#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if lock_count::can_unload() {
        S_OK
    } else {
        S_FALSE
    }
}
