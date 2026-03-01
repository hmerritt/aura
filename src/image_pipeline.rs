use crate::cache::CacheManager;
use crate::config::OutputFormat;
use crate::errors::Result;
use crate::wallpaper::ScreenSpec;
use anyhow::Context;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{crop_imm, resize, FilterType};
use image::{DynamicImage, ImageFormat, ImageReader};
use std::fs;
use std::path::{Path, PathBuf};

pub fn prepare_for_screen(
    input: &Path,
    screen: ScreenSpec,
    cache: &CacheManager,
    format: OutputFormat,
    jpeg_quality: u8,
) -> Result<PathBuf> {
    let key = build_cache_key(input, screen, format, jpeg_quality)?;
    let cached = cache.processed_path_for_key(&key, format);
    if cached.exists() {
        return Ok(cached);
    }

    let source = ImageReader::open(input)
        .with_context(|| format!("failed to open image {}", input.display()))?
        .with_guessed_format()
        .with_context(|| format!("failed to guess image format {}", input.display()))?
        .decode()
        .with_context(|| format!("failed to decode image {}", input.display()))?;

    let resized = cover_scale_center_crop(&source, screen);
    save_output(resized, &cached, format, jpeg_quality)?;
    Ok(cached)
}

fn build_cache_key(
    input: &Path,
    screen: ScreenSpec,
    format: OutputFormat,
    jpeg_quality: u8,
) -> Result<String> {
    let bytes = fs::read(input).with_context(|| format!("failed to read {}", input.display()))?;
    let source_hash = blake3::hash(&bytes).to_hex();
    let key = format!(
        "{}:{}x{}:{}:{}",
        source_hash,
        screen.width,
        screen.height,
        format.extension(),
        jpeg_quality
    );
    Ok(blake3::hash(key.as_bytes()).to_hex().to_string())
}

fn cover_scale_center_crop(image: &DynamicImage, screen: ScreenSpec) -> DynamicImage {
    let src = image.to_rgba8();
    let (src_w, src_h) = src.dimensions();

    let target_ratio = screen.width as f32 / screen.height as f32;
    let src_ratio = src_w as f32 / src_h as f32;

    let cropped = if src_ratio > target_ratio {
        let new_w = ((src_h as f32) * target_ratio).round().max(1.0) as u32;
        let x = (src_w.saturating_sub(new_w)) / 2;
        crop_imm(&src, x, 0, new_w, src_h).to_image()
    } else {
        let new_h = ((src_w as f32) / target_ratio).round().max(1.0) as u32;
        let y = (src_h.saturating_sub(new_h)) / 2;
        crop_imm(&src, 0, y, src_w, new_h).to_image()
    };

    let resized = resize(&cropped, screen.width, screen.height, FilterType::Lanczos3);
    DynamicImage::ImageRgba8(resized)
}

fn save_output(
    image: DynamicImage,
    output_path: &Path,
    format: OutputFormat,
    jpeg_quality: u8,
) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match format {
        OutputFormat::Jpg => {
            let mut file = fs::File::create(output_path)
                .with_context(|| format!("failed to create {}", output_path.display()))?;
            let mut encoder = JpegEncoder::new_with_quality(&mut file, jpeg_quality);
            encoder
                .encode_image(&image)
                .with_context(|| format!("failed to write {}", output_path.display()))?;
        }
        OutputFormat::Png => {
            image
                .save_with_format(output_path, ImageFormat::Png)
                .with_context(|| format!("failed to write {}", output_path.display()))?;
        }
    }
    Ok(())
}
