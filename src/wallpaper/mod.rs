use crate::errors::Result;
use std::path::Path;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

pub trait WallpaperBackend: Send + Sync {
    fn set_wallpaper(&self, path: &Path) -> Result<()>;
}

#[cfg(windows)]
pub fn default_backend() -> Box<dyn WallpaperBackend> {
    Box::new(windows::WindowsWallpaperBackend::new())
}

#[cfg(target_os = "linux")]
pub fn default_backend() -> Box<dyn WallpaperBackend> {
    Box::new(linux::LinuxWallpaperBackend)
}

#[cfg(not(any(windows, target_os = "linux")))]
struct UnsupportedWallpaperBackend;

#[cfg(not(any(windows, target_os = "linux")))]
impl WallpaperBackend for UnsupportedWallpaperBackend {
    fn set_wallpaper(&self, _path: &Path) -> Result<()> {
        anyhow::bail!("wallpaper updates require Windows, GNOME, or KDE Plasma")
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn default_backend() -> Box<dyn WallpaperBackend> {
    Box::new(UnsupportedWallpaperBackend)
}
