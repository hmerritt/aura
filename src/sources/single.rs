use crate::errors::Result;
use crate::sources::{
    image_id, is_supported_image, ImageCandidate, ImageSource, Origin, SourceKind,
};
use async_trait::async_trait;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SingleSource {
    path: PathBuf,
}

impl SingleSource {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl ImageSource for SingleSource {
    fn name(&self) -> &str {
        "single"
    }

    fn kind(&self) -> SourceKind {
        SourceKind::File
    }

    async fn refresh(&mut self) -> Result<Vec<ImageCandidate>> {
        if !self.path.exists() || !self.path.is_file() || !is_supported_image(&self.path) {
            return Ok(Vec::new());
        }
        let metadata = fs::metadata(&self.path)?;
        let mtime = metadata.modified().ok();
        Ok(vec![ImageCandidate {
            id: image_id("file", &self.path),
            origin: Origin::File,
            local_path: self.path.clone(),
            mtime,
        }])
    }
}
