//! `IClassFactory` implementation for the `IExplorerCommand` COM class.

use std::ffi::c_void;
use windows::core::{GUID, HRESULT};

use crate::command::create_handler;
use crate::server::lock_count;

const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_C000_000000000046);
// IClassFactory IID: {00000001-0000-0000-C000-000000000046}
const IID_ICLASS_FACTORY: GUID = GUID::from_u128(0x00000001_0000_0000_C000_000000000046);
// IExplorerCommand IID
const IID_IEXPLORER_COMMAND: GUID = GUID::from_u128(0xa08ce4d0_fa25_44ab_b57c_c7b1c323e0b9);

const E_INVALIDARG: HRESULT = HRESULT(0x80070057u32 as i32);
const E_NOINTERFACE: HRESULT = HRESULT(0x80004002u32 as i32);
const E_NOAGGREGATION: HRESULT = HRESULT(0x80040110u32 as i32);
const S_OK: HRESULT = HRESULT(0);

#[repr(C)]
struct ClassFactoryVtbl {
    query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    create_instance: unsafe extern "system" fn(*mut c_void, *mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
    lock_server: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
}

#[repr(C)]
pub(crate) struct ClassFactory {
    vtable: *const ClassFactoryVtbl,
    ref_count: std::sync::atomic::AtomicI32,
}

static CF_VTBL: ClassFactoryVtbl = ClassFactoryVtbl {
    query_interface: cf_query_interface,
    add_ref: cf_add_ref,
    release: cf_release,
    create_instance: cf_create_instance,
    lock_server: cf_lock_server,
};

unsafe extern "system" fn cf_query_interface(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    if riid.is_null() || ppv.is_null() {
        return E_INVALIDARG;
    }
    let riid = &*riid;
    if *riid == IID_IUNKNOWN || *riid == IID_ICLASS_FACTORY {
        *ppv = this;
        cf_add_ref(this);
        S_OK
    } else {
        *ppv = std::ptr::null_mut();
        E_NOINTERFACE
    }
}

unsafe extern "system" fn cf_add_ref(this: *mut c_void) -> u32 {
    let cf = &mut *(this as *mut ClassFactory);
    cf.ref_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn cf_release(this: *mut c_void) -> u32 {
    let cf = &mut *(this as *mut ClassFactory);
    let new_count = cf.ref_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) as u32 - 1;
    if new_count == 0 {
        drop(Box::from_raw(this as *mut ClassFactory));
    }
    new_count
}

unsafe extern "system" fn cf_create_instance(
    _this: *mut c_void,
    punkouter: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if !punkouter.is_null() {
        return E_NOAGGREGATION;
    }
    if riid.is_null() || ppv.is_null() {
        return E_INVALIDARG;
    }

    let riid = &*riid;
    if *riid != IID_IUNKNOWN && *riid != IID_IEXPLORER_COMMAND {
        *ppv = std::ptr::null_mut();
        return E_NOINTERFACE;
    }

    // Create the Explorer command handler.
    let handler_ptr = create_handler();
    // Return the raw pointer as IUnknown (first field of the vtable).
    *ppv = handler_ptr as *mut c_void;

    lock_count::object_addref();
    S_OK
}

unsafe extern "system" fn cf_lock_server(_this: *mut c_void, flock: i32) -> HRESULT {
    if flock != 0 {
        lock_count::increment();
    } else {
        lock_count::decrement();
    }
    S_OK
}

/// Create a new ClassFactory instance.
pub(crate) fn create_factory() -> *mut ClassFactory {
    Box::into_raw(Box::new(ClassFactory {
        vtable: &CF_VTBL,
        ref_count: std::sync::atomic::AtomicI32::new(1),
    }))
}
