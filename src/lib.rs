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

const PROXY_DLL_NAMES: &[&str] = &[
    "audioses.dll",
    "d3d9.dll",
    "d3d10.dll",
    "d3d11.dll",
    "dinput8.dll",
    "dwmapi.dll",
    "dxgi.dll",
    "glu32.dll",
    "hid.dll",
    "iphlpapi.dll",
    "msasn1.dll",
    "msimg32.dll",
    "mswsock.dll",
    "opengl32.dll",
    "profapi.dll",
    "propsys.dll",
    "textshaping.dll",
    "version.dll",
    "winhttp.dll",
    "wldp.dll",
    "winmm.dll",
    "xinput9_1_0.dll",
];

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
                
                if len == 0 {
                    let exe_len = GetModuleFileNameW(std::ptr::null_mut(), buffer.as_mut_ptr(), buffer.len() as u32);
                    let path_str = OsString::from_wide(&buffer[..exe_len as usize]).into_string().unwrap_or_default();
                    let path = Path::new(&path_str);
                    (path.parent().unwrap_or_else(|| Path::new("")).to_owned(), false)
                } else {
                    let path_str = OsString::from_wide(&buffer[..len as usize]).into_string().unwrap_or_default();
                    let path = Path::new(&path_str);

                    let file_name = path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("")
                        .to_lowercase();

                    let proxy_mode = PROXY_DLL_NAMES.contains(&file_name.as_str());

                    (path.parent().unwrap_or_else(|| Path::new("")).to_owned(), proxy_mode)
                }
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
