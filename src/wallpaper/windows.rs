use crate::errors::Result;
use crate::wallpaper::{ScreenSpec, WallpaperBackend};
use anyhow::{bail, Context};
use std::iter;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;
use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Graphics::Gdi::{EnumDisplaySettingsW, DEVMODEW, ENUM_CURRENT_SETTINGS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, SystemParametersInfoW, SM_CXSCREEN, SM_CYSCREEN, SPIF_SENDCHANGE,
    SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
};

#[derive(Debug, Default)]
pub struct WindowsWallpaperBackend;

impl WindowsWallpaperBackend {
    pub fn new() -> Self {
        Self
    }
}

impl WallpaperBackend for WindowsWallpaperBackend {
    fn set_wallpaper(&self, path: &Path) -> Result<()> {
        enforce_fill_style().context("failed to enforce wallpaper Fill style")?;

        let absolute = path
            .canonicalize()
            .with_context(|| format!("failed to canonicalize {}", path.display()))?;

        let wide: Vec<u16> = absolute
            .as_os_str()
            .encode_wide()
            .chain(iter::once(0))
            .collect();

        let ok = unsafe {
            SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                wide.as_ptr() as *mut _,
                SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
            )
        };
        if ok == 0 {
            bail!("SystemParametersInfoW(SPI_SETDESKWALLPAPER) failed");
        }
        Ok(())
    }

    fn screen_spec(&self) -> Result<ScreenSpec> {
        if let Some(spec) = physical_primary_screen_spec() {
            return Ok(spec);
        }

        tracing::warn!("falling back to logical screen size via GetSystemMetrics");
        let width = unsafe { GetSystemMetrics(SM_CXSCREEN) };
        let height = unsafe { GetSystemMetrics(SM_CYSCREEN) };
        if width <= 0 || height <= 0 {
            bail!("failed to determine screen dimensions");
        }
        Ok(ScreenSpec {
            width: width as u32,
            height: height as u32,
        })
    }
}

fn physical_primary_screen_spec() -> Option<ScreenSpec> {
    let mut dev_mode: DEVMODEW = unsafe { std::mem::zeroed() };
    dev_mode.dmSize = size_of::<DEVMODEW>() as u16;

    let ok = unsafe { EnumDisplaySettingsW(ptr::null(), ENUM_CURRENT_SETTINGS, &mut dev_mode) };
    if ok == 0 {
        return None;
    }

    let width = dev_mode.dmPelsWidth;
    let height = dev_mode.dmPelsHeight;
    if width == 0 || height == 0 {
        return None;
    }

    Some(ScreenSpec { width, height })
}

fn enforce_fill_style() -> Result<()> {
    let mut key: HKEY = ptr::null_mut();
    let subkey = wide_null("Control Panel\\Desktop");
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut key,
        )
    };
    if status != ERROR_SUCCESS {
        bail!("RegOpenKeyExW failed with status {status}");
    }

    let close_result = (|| -> Result<()> {
        write_reg_sz(key, "WallpaperStyle", "10")?;
        write_reg_sz(key, "TileWallpaper", "0")?;
        Ok(())
    })();

    let close_status = unsafe { RegCloseKey(key) };
    if close_status != ERROR_SUCCESS {
        bail!("RegCloseKey failed with status {close_status}");
    }

    close_result
}

fn write_reg_sz(key: HKEY, value_name: &str, value: &str) -> Result<()> {
    let value_name_w = wide_null(value_name);
    let value_w = wide_null(value);
    let data_len = (value_w.len() * size_of::<u16>()) as u32;

    let status = unsafe {
        RegSetValueExW(
            key,
            value_name_w.as_ptr(),
            0,
            REG_SZ,
            value_w.as_ptr() as *const u8,
            data_len,
        )
    };

    if status != ERROR_SUCCESS {
        bail!("RegSetValueExW({value_name}) failed with status {status}");
    }
    Ok(())
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(iter::once(0)).collect()
}
