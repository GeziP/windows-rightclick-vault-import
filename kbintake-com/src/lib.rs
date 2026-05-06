//! KBIntake Windows 11 Explorer COM DLL — `IExplorerCommand` proof of concept.

pub mod command;
pub mod factory;
pub mod reg;
pub mod server;

const DLL_PROCESS_ATTACH: u32 = 1;

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllMain(
    hinst: *mut core::ffi::c_void,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        // Disable per-thread attach/detach calls for this DLL.
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls(
                windows::Win32::Foundation::HINSTANCE(hinst),
            );
        }
    }
    1 // TRUE
}
