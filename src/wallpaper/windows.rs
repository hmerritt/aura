use crate::errors::Result;
use crate::wallpaper::{ScreenSpec, WallpaperBackend};
use anyhow::{bail, Context};
use std::iter;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
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
