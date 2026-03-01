use crate::errors::Result;
use crate::sources::{image_id, is_supported_image, ImageCandidate, ImageSource, Origin};
use async_trait::async_trait;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct DirectorySource {
    path: PathBuf,
    recursive: bool,
    extensions: Option<HashSet<String>>,
}

impl DirectorySource {
    pub fn new(path: PathBuf, recursive: bool, extensions: Vec<String>) -> Self {
        let extensions = if extensions.is_empty() {
            None
        } else {
            Some(
                extensions
                    .into_iter()
                    .map(|ext| ext.trim().trim_start_matches('.').to_ascii_lowercase())
                    .collect(),
            )
        };
        Self {
            path,
            recursive,
            extensions,
        }
    }

    fn extension_allowed(&self, path: &Path) -> bool {
        match &self.extensions {
            None => true,
            Some(allowed) => path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| allowed.contains(&ext.to_ascii_lowercase()))
                .unwrap_or(false),
        }
    }
}

#[async_trait]
impl ImageSource for DirectorySource {
    fn name(&self) -> &str {
        "directory"
    }

    async fn refresh(&mut self) -> Result<Vec<ImageCandidate>> {
        if !self.path.exists() || !self.path.is_dir() {
            return Ok(Vec::new());
        }

        let mut candidates = Vec::new();
        if self.recursive {
            for entry in WalkDir::new(&self.path).into_iter().flatten() {
                let path = entry.path();
                if !path.is_file() || !is_supported_image(path) || !self.extension_allowed(path) {
                    continue;
                }
                let metadata = fs::metadata(path)?;
                candidates.push(ImageCandidate {
                    id: image_id("directory", path),
                    origin: Origin::Directory,
                    local_path: path.to_path_buf(),
                    mtime: metadata.modified().ok(),
                });
            }
        } else {
            for entry in fs::read_dir(&self.path)? {
                let entry = entry?;
                let path = entry.path();
                if !path.is_file() || !is_supported_image(&path) || !self.extension_allowed(&path) {
                    continue;
                }
                let metadata = fs::metadata(&path)?;
                candidates.push(ImageCandidate {
                    id: image_id("directory", &path),
                    origin: Origin::Directory,
                    local_path: path,
                    mtime: metadata.modified().ok(),
                });
            }
        }

        Ok(candidates)
    }
}
