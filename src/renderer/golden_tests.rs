use super::precompiled::{self, ShaderAssets};
use anyhow::{bail, ensure, Context, Result};
use bytemuck::{Pod, Zeroable};
use image::{ColorType, ImageFormat};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc;
use wgpu::util::DeviceExt;

const BASELINE_COMMIT: &str = "fb226d6";
const MANIFEST_SCHEMA: u32 = 1;
const WIDTH: u32 = 128;
const HEIGHT: u32 = 128;
const FORMAT_NAME: &str = "rgba8unorm";
const SSIM_MIN: f64 = 0.985;
const RGB_RMSE_MAX: f64 = 0.030;
const RGB_MAE_MAX: f64 = 0.018;
const HIGH_ERROR_PIXEL_MAX: f64 = 0.02;
const HIGH_ERROR_CHANNEL_THRESHOLD: f64 = 0.12;
const SHADER_NAMES: [&str; 6] = [
    "dither_asci_1",
    "dither_asci_2",
    "dither_warp",
    "gradient_glossy",
    "limestone_cave",
    "silk",
];

#[derive(Clone, Copy, Debug)]
struct CaptureCase {
    name: &'static str,
    time_seconds: f32,
    frame_index: u32,
    mouse_enabled: bool,
    mouse: [f32; 2],
}

const CAPTURE_CASES: [CaptureCase; 2] = [
    CaptureCase {
        name: "initial",
        time_seconds: 0.0,
        frame_index: 0,
        mouse_enabled: false,
        mouse: [0.0, 0.0],
    },
    CaptureCase {
        name: "animated",
        time_seconds: 3.25,
        frame_index: 195,
        mouse_enabled: true,
        mouse: [96.0, 40.0],
    },
];

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GoldenManifest {
    schema_version: u32,
    baseline_commit: String,
    width: u32,
    height: u32,
    format: String,
    captures: Vec<GoldenCapture>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct GoldenCapture {
    shader: String,
    case: String,
    time_seconds: f32,
    frame_index: u32,
    mouse_enabled: bool,
    resolution: [f32; 4],
    mouse: [f32; 4],
    file: String,
    blake3: String,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PortableUniforms {
    time_seconds: f32,
    frame_index: f32,
    mouse_enabled: f32,
    _padding: f32,
    resolution: [f32; 4],
    mouse: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LegacyUniforms {
    time_seconds: f32,
    frame_index: u32,
    mouse_enabled: u32,
    _padding: u32,
    resolution: [f32; 4],
    mouse: [f32; 4],
}

#[derive(Clone, Copy, Debug)]
struct ImageMetrics {
    ssim: f64,
    rgb_rmse: f64,
    rgb_mae: f64,
    high_error_pixels: f64,
    alpha_exact: bool,
}

impl ImageMetrics {
    fn passes(self) -> bool {
        self.ssim >= SSIM_MIN
            && self.rgb_rmse <= RGB_RMSE_MAX
            && self.rgb_mae <= RGB_MAE_MAX
            && self.high_error_pixels <= HIGH_ERROR_PIXEL_MAX
            && self.alpha_exact
    }

    fn summary(self) -> String {
        format!(
            "SSIM={:.6} (min {:.3}), RGB RMSE={:.6} (max {:.3}), RGB MAE={:.6} (max {:.3}), high-error pixels={:.4}% (max {:.2}%), alpha exact={}",
            self.ssim,
            SSIM_MIN,
            self.rgb_rmse,
            RGB_RMSE_MAX,
            self.rgb_mae,
            RGB_MAE_MAX,
            self.high_error_pixels * 100.0,
            HIGH_ERROR_PIXEL_MAX * 100.0,
            self.alpha_exact,
        )
    }
}

struct GoldenGpu {
    _instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GoldenGpu {
    fn new() -> Result<Self> {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::DX12;
        let instance = wgpu::Instance::new(descriptor);

        let fallback_options = wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: true,
            compatible_surface: None,
            apply_limit_buckets: false,
        };
        let adapter = match pollster::block_on(instance.request_adapter(&fallback_options)) {
            Ok(adapter) => adapter,
            Err(fallback_error) => {
                eprintln!(
                    "D3D12 fallback adapter unavailable ({fallback_error}); trying a hardware adapter"
                );
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    force_fallback_adapter: false,
                    ..fallback_options
                }))
                .context("no usable D3D12 adapter is available for shader golden tests")?
            }
        };
        eprintln!("shader golden adapter: {:?}", adapter.get_info());

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("aura-shader-golden-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .context("failed to request the shader golden GPU device")?;

        Ok(Self {
            _instance: instance,
            device,
            queue,
        })
    }

    fn render_portable(&self, shader: &ShaderAssets, case: CaptureCase) -> Result<Vec<u8>> {
        let vertex_words = load_spirv_words(shader.vertex_spirv)?;
        let vertex = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("aura-golden-portable-vertex"),
                source: wgpu::ShaderSource::SpirV(Cow::Owned(vertex_words)),
            });
        let fragment_words = load_spirv_words(shader.fragment_spirv)?;
        let fragment = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("aura-golden-portable-fragment"),
                source: wgpu::ShaderSource::SpirV(Cow::Owned(fragment_words)),
            });
        let uniforms = PortableUniforms {
            time_seconds: case.time_seconds,
            frame_index: case.frame_index as f32,
            mouse_enabled: if case.mouse_enabled { 1.0 } else { 0.0 },
            _padding: 0.0,
            resolution: [WIDTH as f32, HEIGHT as f32, 0.0, 0.0],
            mouse: [case.mouse[0], case.mouse[1], 0.0, 0.0],
        };
        self.render(
            &vertex,
            "main",
            &fragment,
            "main",
            bytemuck::bytes_of(&uniforms),
        )
    }

    fn render_legacy(&self, shader_bytes: &[u8], case: CaptureCase) -> Result<Vec<u8>> {
        let words = load_spirv_words(shader_bytes)?;
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("aura-golden-legacy-shader"),
                source: wgpu::ShaderSource::SpirV(Cow::Owned(words)),
            });
        let uniforms = LegacyUniforms {
            time_seconds: case.time_seconds,
            frame_index: case.frame_index,
            mouse_enabled: u32::from(case.mouse_enabled),
            _padding: 0,
            resolution: [WIDTH as f32, HEIGHT as f32, 0.0, 0.0],
            mouse: [case.mouse[0], case.mouse[1], 0.0, 0.0],
        };
        self.render(
            &shader,
            "vs_main",
            &shader,
            "fs_main",
            bytemuck::bytes_of(&uniforms),
        )
    }

    fn render(
        &self,
        vertex: &wgpu::ShaderModule,
        vertex_entry: &str,
        fragment: &wgpu::ShaderModule,
        fragment_entry: &str,
        uniform_bytes: &[u8],
    ) -> Result<Vec<u8>> {
        let uniform_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("aura-golden-uniforms"),
                contents: uniform_bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("aura-golden-bind-group-layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("aura-golden-bind-group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("aura-golden-pipeline-layout"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("aura-golden-pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: vertex,
                    entry_point: Some(vertex_entry),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: fragment,
                    entry_point: Some(fragment_entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            });

        let extent = wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        };
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("aura-golden-target"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let padded_row_bytes = padded_bytes_per_row(WIDTH);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("aura-golden-readback"),
            size: (padded_row_bytes * HEIGHT as usize) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("aura-golden-encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("aura-golden-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_row_bytes as u32),
                    rows_per_image: Some(HEIGHT),
                },
            },
            extent,
        );
        self.queue.submit([encoder.finish()]);

        let (sender, receiver) = mpsc::channel();
        readback.map_async(wgpu::MapMode::Read, .., move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .context("failed to poll the shader golden GPU device")?;
        receiver
            .recv()
            .context("shader golden readback callback was not delivered")?
            .context("failed to map the shader golden readback buffer")?;
        let mapped = readback
            .get_mapped_range(..)
            .context("failed to access the shader golden readback buffer")?;
        let pixels = strip_row_padding(&mapped, WIDTH, HEIGHT, padded_row_bytes)?;
        drop(mapped);
        readback.unmap();
        Ok(pixels)
    }
}

fn golden_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("shaders")
}

fn manifest_path() -> PathBuf {
    golden_root().join("manifest.json")
}

fn failure_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("shader-golden-failures")
}

fn load_manifest() -> Result<GoldenManifest> {
    let path = manifest_path();
    let bytes = fs::read(&path)
        .with_context(|| format!("failed to read shader golden manifest {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse shader golden manifest {}", path.display()))
}

fn validate_manifest_header(manifest: &GoldenManifest) -> Result<()> {
    ensure!(
        manifest.schema_version == MANIFEST_SCHEMA,
        "shader golden manifest schema must be {MANIFEST_SCHEMA}"
    );
    ensure!(
        manifest.baseline_commit == BASELINE_COMMIT,
        "shader golden baseline must come from commit {BASELINE_COMMIT}"
    );
    ensure!(
        (manifest.width, manifest.height) == (WIDTH, HEIGHT),
        "shader golden dimensions must be {WIDTH}x{HEIGHT}"
    );
    ensure!(
        manifest.format == FORMAT_NAME,
        "shader golden format must be {FORMAT_NAME}"
    );
    Ok(())
}

fn validate_manifest(manifest: &GoldenManifest, verify_files: bool) -> Result<()> {
    validate_manifest_header(manifest)?;
    ensure!(
        manifest.captures.len() == SHADER_NAMES.len() * CAPTURE_CASES.len(),
        "shader golden manifest must contain exactly 12 captures"
    );
    ensure!(
        precompiled::shader_names() == SHADER_NAMES,
        "embedded shader registry does not match the golden shader set"
    );

    let mut seen = BTreeSet::new();
    for capture in &manifest.captures {
        ensure!(
            SHADER_NAMES.contains(&capture.shader.as_str()),
            "unknown shader in golden manifest: {}",
            capture.shader
        );
        let case = CAPTURE_CASES
            .iter()
            .find(|candidate| candidate.name == capture.case)
            .with_context(|| {
                format!("unknown capture case in golden manifest: {}", capture.case)
            })?;
        ensure!(
            seen.insert((capture.shader.clone(), capture.case.clone())),
            "duplicate golden capture {}/{}",
            capture.shader,
            capture.case
        );
        ensure!(
            capture.time_seconds == case.time_seconds,
            "unexpected capture time"
        );
        ensure!(
            capture.frame_index == case.frame_index,
            "unexpected frame index"
        );
        ensure!(
            capture.mouse_enabled == case.mouse_enabled,
            "unexpected mouse-enabled value"
        );
        ensure!(
            capture.resolution == [WIDTH as f32, HEIGHT as f32, 0.0, 0.0],
            "unexpected capture resolution"
        );
        ensure!(
            capture.mouse == [case.mouse[0], case.mouse[1], 0.0, 0.0],
            "unexpected capture mouse coordinates"
        );
        let relative = Path::new(&capture.file);
        ensure!(
            relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
                && relative.extension().and_then(|value| value.to_str()) == Some("png"),
            "golden capture path must be a relative PNG path: {}",
            capture.file
        );
        ensure!(
            relative == Path::new(&capture.shader).join(format!("{}.png", capture.case)),
            "golden capture path does not match its shader and case: {}",
            capture.file
        );
        ensure!(capture.blake3.len() == 64, "invalid golden BLAKE3 hash");

        if verify_files {
            let path = golden_root().join(relative);
            let bytes = fs::read(&path)
                .with_context(|| format!("failed to read golden image {}", path.display()))?;
            ensure!(
                blake3::hash(&bytes).to_hex().as_str() == capture.blake3,
                "golden image hash does not match manifest: {}",
                path.display()
            );
            let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
                .with_context(|| format!("failed to decode golden image {}", path.display()))?
                .to_rgba8();
            ensure!(
                image.dimensions() == (WIDTH, HEIGHT),
                "golden image has unexpected dimensions: {}",
                path.display()
            );
        }
    }
    Ok(())
}

fn load_spirv_words(bytes: &[u8]) -> Result<Vec<u32>> {
    ensure!(
        bytes.len().is_multiple_of(4),
        "shader binary size is not a multiple of four"
    );
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn padded_bytes_per_row(width: u32) -> usize {
    let unpadded = width as usize * 4;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    unpadded.div_ceil(alignment) * alignment
}

fn strip_row_padding(
    bytes: &[u8],
    width: u32,
    height: u32,
    padded_row_bytes: usize,
) -> Result<Vec<u8>> {
    let row_bytes = width as usize * 4;
    ensure!(
        padded_row_bytes >= row_bytes,
        "padded row length is shorter than the image row"
    );
    ensure!(
        bytes.len() == padded_row_bytes * height as usize,
        "GPU readback length does not match its padded dimensions"
    );
    let mut pixels = Vec::with_capacity(row_bytes * height as usize);
    for row in bytes.chunks_exact(padded_row_bytes) {
        pixels.extend_from_slice(&row[..row_bytes]);
    }
    Ok(pixels)
}

fn compare_images(expected: &[u8], actual: &[u8], width: u32, height: u32) -> Result<ImageMetrics> {
    let expected_len = width as usize * height as usize * 4;
    ensure!(
        expected.len() == expected_len,
        "unexpected baseline pixel length"
    );
    ensure!(
        actual.len() == expected_len,
        "unexpected actual pixel length"
    );

    let mut squared_error = 0.0;
    let mut absolute_error = 0.0;
    let mut high_error_pixels = 0usize;
    let mut alpha_exact = true;
    for (expected_pixel, actual_pixel) in expected.chunks_exact(4).zip(actual.chunks_exact(4)) {
        let mut maximum_channel_error = 0.0_f64;
        for channel in 0..3 {
            let difference =
                (f64::from(expected_pixel[channel]) - f64::from(actual_pixel[channel])) / 255.0;
            squared_error += difference * difference;
            absolute_error += difference.abs();
            maximum_channel_error = maximum_channel_error.max(difference.abs());
        }
        high_error_pixels += usize::from(maximum_channel_error > HIGH_ERROR_CHANNEL_THRESHOLD);
        alpha_exact &= expected_pixel[3] == actual_pixel[3];
    }

    let pixel_count = f64::from(width) * f64::from(height);
    Ok(ImageMetrics {
        ssim: windowed_luma_ssim(expected, actual, width, height),
        rgb_rmse: (squared_error / (pixel_count * 3.0)).sqrt(),
        rgb_mae: absolute_error / (pixel_count * 3.0),
        high_error_pixels: high_error_pixels as f64 / pixel_count,
        alpha_exact,
    })
}

fn windowed_luma_ssim(expected: &[u8], actual: &[u8], width: u32, height: u32) -> f64 {
    const WINDOW: u32 = 8;
    const C1: f64 = 0.0001;
    const C2: f64 = 0.0009;

    let mut total = 0.0;
    let mut windows = 0usize;
    for top in (0..height).step_by(WINDOW as usize) {
        for left in (0..width).step_by(WINDOW as usize) {
            let right = (left + WINDOW).min(width);
            let bottom = (top + WINDOW).min(height);
            let count = f64::from((right - left) * (bottom - top));
            let mut expected_mean = 0.0;
            let mut actual_mean = 0.0;
            for y in top..bottom {
                for x in left..right {
                    expected_mean += luma(expected, width, x, y);
                    actual_mean += luma(actual, width, x, y);
                }
            }
            expected_mean /= count;
            actual_mean /= count;

            let mut expected_variance = 0.0;
            let mut actual_variance = 0.0;
            let mut covariance = 0.0;
            for y in top..bottom {
                for x in left..right {
                    let expected_delta = luma(expected, width, x, y) - expected_mean;
                    let actual_delta = luma(actual, width, x, y) - actual_mean;
                    expected_variance += expected_delta * expected_delta;
                    actual_variance += actual_delta * actual_delta;
                    covariance += expected_delta * actual_delta;
                }
            }
            expected_variance /= count;
            actual_variance /= count;
            covariance /= count;
            total += ((2.0 * expected_mean * actual_mean + C1) * (2.0 * covariance + C2))
                / ((expected_mean * expected_mean + actual_mean * actual_mean + C1)
                    * (expected_variance + actual_variance + C2));
            windows += 1;
        }
    }
    total / windows as f64
}

fn luma(pixels: &[u8], width: u32, x: u32, y: u32) -> f64 {
    let offset = ((y * width + x) * 4) as usize;
    (0.2126 * f64::from(pixels[offset])
        + 0.7152 * f64::from(pixels[offset + 1])
        + 0.0722 * f64::from(pixels[offset + 2]))
        / 255.0
}

fn save_rgba(path: &Path, pixels: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    image::save_buffer_with_format(
        path,
        pixels,
        WIDTH,
        HEIGHT,
        ColorType::Rgba8,
        ImageFormat::Png,
    )
    .with_context(|| format!("failed to write {}", path.display()))
}

fn save_failure_images(shader: &str, case: &str, expected: &[u8], actual: &[u8]) -> Result<()> {
    let directory = failure_root().join(shader);
    save_rgba(&directory.join(format!("{case}-expected.png")), expected)?;
    save_rgba(&directory.join(format!("{case}-actual.png")), actual)?;
    let difference = amplified_difference(expected, actual);
    save_rgba(&directory.join(format!("{case}-diff.png")), &difference)
}

fn amplified_difference(expected: &[u8], actual: &[u8]) -> Vec<u8> {
    expected
        .chunks_exact(4)
        .zip(actual.chunks_exact(4))
        .flat_map(|(expected_pixel, actual_pixel)| {
            let channel = |index: usize| {
                expected_pixel[index]
                    .abs_diff(actual_pixel[index])
                    .saturating_mul(4)
            };
            [channel(0), channel(1), channel(2), 255]
        })
        .collect()
}

fn current_shader(name: &str) -> Result<&'static ShaderAssets> {
    precompiled::shader_assets(name)
        .with_context(|| format!("portable shader {name} is not embedded"))
}

fn capture_from_case(
    shader: &str,
    case: CaptureCase,
    file: String,
    blake3: String,
) -> GoldenCapture {
    GoldenCapture {
        shader: shader.to_string(),
        case: case.name.to_string(),
        time_seconds: case.time_seconds,
        frame_index: case.frame_index,
        mouse_enabled: case.mouse_enabled,
        resolution: [WIDTH as f32, HEIGHT as f32, 0.0, 0.0],
        mouse: [case.mouse[0], case.mouse[1], 0.0, 0.0],
        file,
        blake3,
    }
}

#[test]
fn strips_gpu_row_padding() -> Result<()> {
    let padded = 256;
    let mut bytes = vec![0xEE; padded * 2];
    bytes[..12].copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
    bytes[padded..padded + 12].copy_from_slice(&[12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23]);
    assert_eq!(
        strip_row_padding(&bytes, 3, 2, padded)?,
        (0_u8..24).collect::<Vec<_>>()
    );
    Ok(())
}

#[test]
fn identical_images_have_perfect_metrics() -> Result<()> {
    let pixels = vec![127; 16 * 16 * 4];
    let metrics = compare_images(&pixels, &pixels, 16, 16)?;
    assert!((metrics.ssim - 1.0).abs() < f64::EPSILON);
    assert_eq!(metrics.rgb_rmse, 0.0);
    assert_eq!(metrics.rgb_mae, 0.0);
    assert_eq!(metrics.high_error_pixels, 0.0);
    assert!(metrics.alpha_exact);
    assert!(metrics.passes());
    Ok(())
}

#[test]
fn perceptual_thresholds_reject_color_changes() -> Result<()> {
    let expected = vec![128; 16 * 16 * 4];
    let mut actual = expected.clone();
    for pixel in actual.chunks_exact_mut(4) {
        pixel[0] = 208;
    }
    assert!(!compare_images(&expected, &actual, 16, 16)?.passes());
    Ok(())
}

#[test]
fn perceptual_thresholds_reject_geometry_changes() -> Result<()> {
    let mut expected = vec![0; 16 * 16 * 4];
    let mut actual = vec![0; expected.len()];
    for y in 0..16 {
        for x in 0..16 {
            let expected_offset = (y * 16 + x) * 4;
            let actual_offset = (y * 16 + ((x + 4) % 16)) * 4;
            let value = if x < 8 { 255 } else { 0 };
            expected[expected_offset..expected_offset + 4]
                .copy_from_slice(&[value, value, value, 255]);
            actual[actual_offset..actual_offset + 4].copy_from_slice(&[value, value, value, 255]);
        }
    }
    assert!(!compare_images(&expected, &actual, 16, 16)?.passes());
    Ok(())
}

#[test]
fn golden_manifest_covers_every_shader_and_case() -> Result<()> {
    validate_manifest(&load_manifest()?, true)
}

#[test]
#[ignore = "requires a Windows D3D12 adapter; run explicitly in Windows CI"]
fn portable_shaders_match_legacy_goldens() -> Result<()> {
    let manifest = load_manifest()?;
    validate_manifest(&manifest, true)?;
    let gpu = GoldenGpu::new()?;
    let mut failures = Vec::new();

    for capture in &manifest.captures {
        let case = CAPTURE_CASES
            .iter()
            .copied()
            .find(|candidate| candidate.name == capture.case)
            .context("validated capture case disappeared")?;
        let expected = image::open(golden_root().join(&capture.file))
            .with_context(|| format!("failed to load golden image {}", capture.file))?
            .to_rgba8()
            .into_raw();
        let actual = gpu.render_portable(current_shader(&capture.shader)?, case)?;
        let metrics = compare_images(&expected, &actual, WIDTH, HEIGHT)?;
        eprintln!("{}/{}: {}", capture.shader, capture.case, metrics.summary());
        if !metrics.passes() {
            save_failure_images(&capture.shader, &capture.case, &expected, &actual)?;
            failures.push(format!(
                "{}/{}: {}",
                capture.shader,
                capture.case,
                metrics.summary()
            ));
        }
    }

    if !failures.is_empty() {
        bail!(
            "portable shaders differ from the Rust-GPU baseline:\n{}\nartifacts: {}",
            failures.join("\n"),
            failure_root().display()
        );
    }
    Ok(())
}

#[test]
#[ignore = "maintainer-only baseline regeneration"]
fn regenerate_legacy_shader_goldens() -> Result<()> {
    ensure!(
        std::env::var("AURA_UPDATE_SHADER_GOLDENS").as_deref() == Ok("1"),
        "set AURA_UPDATE_SHADER_GOLDENS=1 to authorize baseline regeneration"
    );
    let legacy_directory = PathBuf::from(
        std::env::var_os("AURA_LEGACY_SHADER_DIR")
            .context("set AURA_LEGACY_SHADER_DIR to the flat directory of fb226d6 .spv files")?,
    );
    validate_manifest_header(&load_manifest()?)?;

    let gpu = GoldenGpu::new()?;
    let mut rendered = Vec::new();
    let mut failures = Vec::new();
    for shader_name in SHADER_NAMES {
        let legacy_path = legacy_directory.join(format!("{shader_name}.spv"));
        let legacy_shader = fs::read(&legacy_path).with_context(|| {
            format!(
                "failed to read legacy shader built from {BASELINE_COMMIT}: {}",
                legacy_path.display()
            )
        })?;
        for case in CAPTURE_CASES {
            let expected = gpu.render_legacy(&legacy_shader, case)?;
            let actual = gpu.render_portable(current_shader(shader_name)?, case)?;
            let metrics = compare_images(&expected, &actual, WIDTH, HEIGHT)?;
            eprintln!("{shader_name}/{}: {}", case.name, metrics.summary());
            if !metrics.passes() {
                save_failure_images(shader_name, case.name, &expected, &actual)?;
                failures.push(format!(
                    "{shader_name}/{}: {}",
                    case.name,
                    metrics.summary()
                ));
            }
            rendered.push((shader_name, case, expected));
        }
    }
    if !failures.is_empty() {
        bail!(
            "refusing to update goldens because portable shaders do not match {BASELINE_COMMIT}:\n{}\nartifacts: {}",
            failures.join("\n"),
            failure_root().display()
        );
    }

    let mut captures = Vec::with_capacity(rendered.len());
    for (shader_name, case, pixels) in rendered {
        let relative = format!("{shader_name}/{}.png", case.name);
        let path = golden_root().join(Path::new(&relative));
        save_rgba(&path, &pixels)?;
        let encoded = fs::read(&path)
            .with_context(|| format!("failed to hash generated golden {}", path.display()))?;
        captures.push(capture_from_case(
            shader_name,
            case,
            relative,
            blake3::hash(&encoded).to_hex().to_string(),
        ));
    }
    let manifest = GoldenManifest {
        schema_version: MANIFEST_SCHEMA,
        baseline_commit: BASELINE_COMMIT.to_string(),
        width: WIDTH,
        height: HEIGHT,
        format: FORMAT_NAME.to_string(),
        captures,
    };
    validate_manifest(&manifest, true)?;
    let mut json = serde_json::to_string_pretty(&manifest)?;
    json.push('\n');
    fs::write(manifest_path(), json).context("failed to write shader golden manifest")?;
    Ok(())
}
