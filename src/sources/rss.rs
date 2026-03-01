use crate::errors::Result;
use crate::sources::{image_id, ImageCandidate, ImageSource, Origin, SourceKind};
use anyhow::Context;
use async_trait::async_trait;
use feed_rs::parser;
use regex::Regex;
use reqwest::header::CONTENT_TYPE;
use reqwest::Client;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone)]
pub struct RssSource {
    url: String,
    max_items: usize,
    download_dir: PathBuf,
    client: Client,
    html_img_regex: Regex,
}

impl RssSource {
    pub fn new(url: String, max_items: usize, download_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&download_dir)
            .with_context(|| format!("failed to create {}", download_dir.display()))?;

        let client = Client::builder()
            .user_agent("bgm/0.1 (+https://example.invalid)")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            url,
            max_items,
            download_dir,
            client,
            html_img_regex: Regex::new(r#"https?://[^"'\s>]+\.(jpg|jpeg|png|gif|bmp|webp)"#)?,
        })
    }

    fn extract_feed_image_urls(&self, body: &[u8]) -> Result<Vec<String>> {
        let feed = parser::parse(body)?;
        let mut urls = Vec::new();
        let mut seen = HashSet::new();

        for entry in feed.entries {
            for link in entry.links {
                if looks_like_image_url(&link.href) && seen.insert(link.href.clone()) {
                    urls.push(link.href);
                }
            }

            if let Some(summary) = entry.summary {
                for image_url in self.find_urls_in_text(&summary.content) {
                    if seen.insert(image_url.clone()) {
                        urls.push(image_url);
                    }
                }
            }

            if let Some(content) = entry.content {
                if let Some(body) = content.body {
                    for image_url in self.find_urls_in_text(&body) {
                        if seen.insert(image_url.clone()) {
                            urls.push(image_url);
                        }
                    }
                }
            }
        }

        Ok(urls)
    }

    fn find_urls_in_text(&self, text: &str) -> Vec<String> {
        self.html_img_regex
            .find_iter(text)
            .map(|m| m.as_str().to_string())
            .collect()
    }

    async fn download_image(&self, image_url: &str) -> Result<Option<PathBuf>> {
        let response = self
            .client
            .get(image_url)
            .send()
            .await
            .with_context(|| format!("failed request for {image_url}"))?;
        let response = response
            .error_for_status()
            .with_context(|| format!("feed image url returned non-success for {image_url}"))?;

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .map(|v| v.to_ascii_lowercase());

        if let Some(content_type) = &content_type {
            if !content_type.starts_with("image/") && !looks_like_image_url(image_url) {
                return Ok(None);
            }
        }

        let ext = extension_from_content_type(content_type.as_deref())
            .or_else(|| extension_from_url(image_url))
            .unwrap_or_else(|| "jpg".to_string());

        let filename = format!("{}.{}", blake3::hash(image_url.as_bytes()).to_hex(), ext);
        let output = self.download_dir.join(filename);
        if output.exists() && output.is_file() {
            return Ok(Some(output));
        }

        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("failed reading bytes from {image_url}"))?;
        if bytes.is_empty() {
            return Ok(None);
        }

        tokio::fs::write(&output, &bytes)
            .await
            .with_context(|| format!("failed to write {}", output.display()))?;
        Ok(Some(output))
    }
}

#[async_trait]
impl ImageSource for RssSource {
    fn name(&self) -> &str {
        "rss"
    }

    fn kind(&self) -> SourceKind {
        SourceKind::Rss
    }

    async fn refresh(&mut self) -> Result<Vec<ImageCandidate>> {
        let body = self
            .client
            .get(&self.url)
            .send()
            .await
            .with_context(|| format!("failed request for {}", self.url))?
            .error_for_status()
            .with_context(|| format!("feed returned non-success for {}", self.url))?
            .bytes()
            .await
            .with_context(|| format!("failed reading feed bytes from {}", self.url))?;

        let feed_urls = self.extract_feed_image_urls(&body)?;
        let mut candidates = Vec::new();

        for image_url in feed_urls.into_iter().take(self.max_items) {
            match self.download_image(&image_url).await {
                Ok(Some(path)) => {
                    let mtime = fs::metadata(&path)
                        .ok()
                        .and_then(|meta| meta.modified().ok());
                    candidates.push(ImageCandidate {
                        id: image_id("rss", &PathBuf::from(&image_url)),
                        origin: Origin::Rss,
                        local_path: path,
                        mtime,
                    });
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(url = %image_url, error = %error, "failed to download RSS image");
                }
            }
        }

        Ok(candidates)
    }
}

fn looks_like_image_url(url: &str) -> bool {
    extension_from_url(url)
        .map(|ext| {
            matches!(
                ext.as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp"
            )
        })
        .unwrap_or(false)
}

fn extension_from_url(url: &str) -> Option<String> {
    Url::parse(url).ok().and_then(|parsed| {
        Path::new(parsed.path())
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|x| x.to_ascii_lowercase())
    })
}

fn extension_from_content_type(content_type: Option<&str>) -> Option<String> {
    let content_type = content_type?;
    if content_type.starts_with("image/jpeg") || content_type.starts_with("image/jpg") {
        Some("jpg".to_string())
    } else if content_type.starts_with("image/png") {
        Some("png".to_string())
    } else if content_type.starts_with("image/gif") {
        Some("gif".to_string())
    } else if content_type.starts_with("image/webp") {
        Some("webp".to_string())
    } else if content_type.starts_with("image/bmp") {
        Some("bmp".to_string())
    } else {
        None
    }
}
