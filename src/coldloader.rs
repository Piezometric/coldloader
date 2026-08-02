use std::{path::PathBuf, sync::LazyLock};
use anyhow::{anyhow, Result};
use winapi::um::{libloaderapi::LoadLibraryW, processthreadsapi::GetCurrentProcessId};

use crate::ini::CONFIG;

static STEAM_UNIVERSE: &str = "public";
static PROCESS_ID: LazyLock<u32> = LazyLock::new(|| unsafe {
    GetCurrentProcessId()
});

const STEAMCLIENT_KEY: &str = if cfg!(target_pointer_width = "64") {
    "SteamClientDll64"
} else {
    "SteamClientDll"
};

fn patch_registry() -> Result<()> {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey_with_flags("Software\\Valve\\Steam\\ActiveProcess", winreg::enums::KEY_ALL_ACCESS)
        .map(|key| {
            let steamclient_path = CONFIG.steamclient_path
                .to_string_lossy()
                .to_string();
            
            let _ = key.set_value("pid", &*PROCESS_ID);
            let _ = key.set_value("Universe", &STEAM_UNIVERSE);
            let _ = key.set_value(STEAMCLIENT_KEY, &steamclient_path);
        })
        .map_err(|e| {
            log::error!("Unable to patch Registry (HKCU), error = {:?}", e);
            e
        })?;

    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey_with_flags("Software\\Valve\\Steam", winreg::enums::KEY_ALL_ACCESS)
        .map(|key| {
            let steam_path = CONFIG.steamclient_path
                .parent()
                .unwrap()
                .to_string_lossy()
                .to_string();
            
            let _ = key.set_value("SteamPath", &steam_path);
            let _ = key.set_value("RunningAppID", &CONFIG.app_id);
        })
        .map_err(|e| {
            log::error!("Unable to patch Registry (HKCU #2), error = {:?}", e);
            e
        })?;

    Ok(())
}

fn set_steam_env_vars(app_id: u32) {
    unsafe {
        std::env::set_var("SteamAppId", app_id.to_string());
        std::env::set_var("SteamGameId", app_id.to_string());
        std::env::set_var("SteamClientLaunch", "1");
        std::env::set_var("SteamEnv", "1");
    }

    log::info!("Set Steam environment variables");
}

pub fn initialize() -> Result<()> {
    set_steam_env_vars(CONFIG.app_id);

    patch_registry()?;
    
    let client_path = CONFIG.steamclient_path.to_string_lossy().to_string();
    log::info!("Loading steamclient from path: {}", client_path);
    let client_dll = unsafe {
        let path: Vec<u16> = client_path.encode_utf16().chain(std::iter::once(0)).collect();
        LoadLibraryW(path.as_ptr())
    };
    if client_dll.is_null() {
        return Err(anyhow!("Failed to load steamclient"));
    }

    log::info!("Loaded steamclient");

    let overlay_dll_name = if cfg!(target_pointer_width = "64") {
        "gameoverlayrenderer64.dll"
    } else {
        "gameoverlayrenderer.dll"
    };

    let gameoverlay_path = std::env::current_exe().unwrap().parent().unwrap().join(overlay_dll_name);
    if gameoverlay_path.exists() {
        let gameoverlay_dll = unsafe {
            let path: Vec<u16> = gameoverlay_path.to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
            LoadLibraryW(path.as_ptr())
        };
        if gameoverlay_dll.is_null() {
            return Err(anyhow!("Failed to load {}", overlay_dll_name));
        }

        log::info!("Loaded {}", overlay_dll_name);
    } else {
        log::warn!("{} not found, skipping load", overlay_dll_name);
    }

    Ok(())
}

pub fn cleanup() -> Result<()> {
    let install_path = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags("Software\\WOW6432Node\\Valve\\Steam", winreg::enums::KEY_ALL_ACCESS)
        .and_then(|key| {
            let path = key.get_value::<String, _>("InstallPath")?;
            Ok(PathBuf::from(path))
        });

    if let Ok(path) = install_path {
        let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags("Software\\Valve\\Steam\\ActiveProcess", winreg::enums::KEY_ALL_ACCESS)
            .map(|key| {
                let steamclient_dll = if cfg!(target_pointer_width = "64") {
                    path.join("steamclient64.dll")
                } else {
                    path.join("steamclient.dll")
                };

                let _ = key.set_value(STEAMCLIENT_KEY, &steamclient_dll.to_string_lossy().to_string());
            });
        
        let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey_with_flags("Software\\Valve\\Steam", winreg::enums::KEY_ALL_ACCESS)
            .map(|key| {
                let _ = key.set_value("SteamPath", &path.to_string_lossy().to_string());
            });
    }

    log::info!("Cleaned up registry");
    Ok(())
}
