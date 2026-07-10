use crate::errors::Result;
use crate::macos_app;
use crate::wallpaper::WallpaperBackend;
use std::path::Path;

pub struct MacWallpaperBackend;

impl WallpaperBackend for MacWallpaperBackend {
    fn set_wallpaper(&self, path: &Path) -> Result<()> {
        macos_app::set_wallpaper(path)
    }
}
