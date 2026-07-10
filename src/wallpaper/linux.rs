use crate::errors::Result;
use crate::linux_desktop;
use crate::wallpaper::WallpaperBackend;
use std::path::Path;

#[derive(Debug, Default)]
pub struct LinuxWallpaperBackend;

impl WallpaperBackend for LinuxWallpaperBackend {
    fn set_wallpaper(&self, path: &Path) -> Result<()> {
        linux_desktop::publish_image(path)
    }
}
