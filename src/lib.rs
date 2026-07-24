mod proxy;
mod exports;
mod ini;
mod logging;
pub mod coldloader;

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use winapi::shared::minwindef::{BOOL, DWORD, HMODULE, LPVOID, TRUE};
use winapi::um::libloaderapi::GetModuleFileNameW;
use winapi::um::winnt::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};

pub static DLL_PATH: OnceLock<PathBuf> = OnceLock::new();
static IS_PROXY_MODE: OnceLock<bool> = OnceLock::new();

#[unsafe(no_mangle)]
#[allow(non_snake_case)]
unsafe extern "system" fn DllMain(
    module: HMODULE,
    call_reason: DWORD,
    _reserved: LPVOID,
) -> BOOL {
    match call_reason {
        DLL_PROCESS_ATTACH => {
            let (dll_path, is_proxy) = {
                let mut buffer = [0u16; 1024];
                let len = GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32);
                let path_str = OsString::from_wide(&buffer[..len as usize]).into_string().unwrap();
                let path = Path::new(&path_str);

                let file_name = path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let proxy_mode = file_name != "coldloader.dll";
                (path.parent().unwrap().to_owned(), proxy_mode)
            };

            DLL_PATH.set(dll_path).ok();
            IS_PROXY_MODE.set(is_proxy).ok();

            logging::init_logger(is_proxy);
            logging::setup_panic_handler();

            std::thread::spawn(|| {
                if let Err(e) = coldloader::initialize() {
                    log::error!("Failed to initialize: {}", e);
                    logging::message_box(&format!("Failed to initialize ColdLoader:\n{}", e));
                    std::process::exit(1);
                }

                std::thread::sleep(std::time::Duration::from_secs(ini::CONFIG.cleanup_delay));
                let _ = coldloader::cleanup();
            });

            TRUE
        }
        DLL_PROCESS_DETACH => {
            let _ = coldloader::cleanup();

            if *IS_PROXY_MODE.get().unwrap_or(&false) {
                unsafe { proxy::cleanup_proxied_dll() };
            }

            TRUE
        }
        _ => TRUE,
    }
}
