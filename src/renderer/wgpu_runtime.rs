use super::desktop_windows::DesktopRect;
use crate::config::{ShaderColorSpace, ShaderConfig};
use crate::errors::Result;
use anyhow::{anyhow, bail, Context};
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

const SHADER_POWER_PREFERENCE: wgpu::PowerPreference = wgpu::PowerPreference::LowPower;
const SHADER_MAX_FRAME_LATENCY: u8 = 1;
const SHADER_MEMORY_TARGET_MB: u64 = 80;
const COMPOSITE_SHADER_SOURCE: &str = r#"
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 2.0),
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = uvs[vertex_index];
    return output;
}

@group(0) @binding(0) var source_texture: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(source_texture, source_sampler, input.uv);
}
"#;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct ShaderUniforms {
    time_seconds: f32,
    frame_index: u32,
    mouse_enabled: u32,
    _padding: u32,
    resolution: [f32; 4],
    mouse: [f32; 4],
}

struct InternalTarget {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    size: PhysicalSize<u32>,
}

pub struct WgpuRuntime {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    scene_pipeline: wgpu::RenderPipeline,
    scene_bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    composite_pipeline: wgpu::RenderPipeline,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    composite_sampler: wgpu::Sampler,
    internal_target: InternalTarget,
    started_at: Instant,
    frame_index: u32,
    mouse_enabled: bool,
    resolution_percent: u8,
}

impl WgpuRuntime {
    pub fn new(
        window: Arc<Window>,
        shader_bytes: &[u8],
        shader_config: ShaderConfig,
        desktop_rect: DesktopRect,
    ) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| anyhow!("failed to create wgpu surface: {error}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: SHADER_POWER_PREFERENCE,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .ok_or_else(|| anyhow!("failed to find a compatible GPU adapter"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("aura-shader-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            },
            None,
        ))
        .context("failed to request GPU device")?;

        let caps = surface.get_capabilities(&adapter);
        if caps.formats.is_empty() {
            bail!("adapter reported no surface formats");
        }
        let format = pick_surface_format(shader_config.color_space, &caps.formats);
        if shader_config.color_space == ShaderColorSpace::Unorm && format.is_srgb() {
            tracing::warn!(
                color_space = ?shader_config.color_space,
                surface_format = ?format,
                "non-srgb surface format was requested but unavailable; falling back to sRGB"
            );
        }
        tracing::info!(
            color_space = ?shader_config.color_space,
            surface_format = ?format,
            "shader runtime surface color format selected"
        );
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);

        let output_size = PhysicalSize::new(
            desktop_rect.width.max(1) as u32,
            desktop_rect.height.max(1) as u32,
        );
        let internal_size = compute_internal_render_size(output_size, shader_config.resolution);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: output_size.width,
            height: output_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: u32::from(SHADER_MAX_FRAME_LATENCY),
        };

        let estimated_bytes =
            estimate_swapchain_memory_bytes(config.width, config.height, SHADER_MAX_FRAME_LATENCY);
        let estimated_mb = bytes_to_megabytes(estimated_bytes);
        tracing::info!(
            output_width = output_size.width,
            output_height = output_size.height,
            internal_width = internal_size.width,
            internal_height = internal_size.height,
            resolution_percent = shader_config.resolution,
            desktop_scope = ?shader_config.desktop_scope,
            power_preference = ?SHADER_POWER_PREFERENCE,
            max_frame_latency = SHADER_MAX_FRAME_LATENCY,
            memory_target_mb = SHADER_MEMORY_TARGET_MB,
            estimated_swapchain_mb = estimated_mb,
            "shader runtime surface memory estimate"
        );
        if estimated_mb > SHADER_MEMORY_TARGET_MB as f64 {
            tracing::warn!(
                memory_target_mb = SHADER_MEMORY_TARGET_MB,
                estimated_swapchain_mb = estimated_mb,
                "shader swapchain estimate exceeds memory target; continuing shader mode"
            );
        }

        surface.configure(&device, &config);

        let (uniform_buffer, scene_bind_group, scene_pipeline) = create_scene_pipeline(
            &device,
            config.format,
            shader_bytes,
            shader_config.mouse_enabled,
            internal_size,
        )?;
        let composite_bind_group_layout = create_composite_bind_group_layout(&device);
        let composite_sampler = create_composite_sampler(&device);
        let composite_pipeline =
            create_composite_pipeline(&device, config.format, &composite_bind_group_layout);
        let internal_target = create_internal_target(
            &device,
            internal_size,
            config.format,
            &composite_bind_group_layout,
            &composite_sampler,
        );

        Ok(Self {
            _instance: instance,
            surface,
            device,
            queue,
            config,
            scene_pipeline,
            scene_bind_group,
            uniform_buffer,
            composite_pipeline,
            composite_bind_group_layout,
            composite_sampler,
            internal_target,
            started_at: Instant::now(),
            frame_index: 0,
            mouse_enabled: shader_config.mouse_enabled,
            resolution_percent: shader_config.resolution,
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
        self.recreate_internal_target();
    }

    pub fn apply_config(
        &mut self,
        shader_bytes: &[u8],
        shader_config: ShaderConfig,
        desktop_rect: DesktopRect,
    ) -> Result<()> {
        let adapter = pollster::block_on(self._instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: SHADER_POWER_PREFERENCE,
                force_fallback_adapter: false,
                compatible_surface: Some(&self.surface),
            },
        ))
        .ok_or_else(|| anyhow!("failed to find a compatible GPU adapter"))?;

        let caps = self.surface.get_capabilities(&adapter);
        if caps.formats.is_empty() {
            bail!("adapter reported no surface formats");
        }
        let format = pick_surface_format(shader_config.color_space, &caps.formats);
        if shader_config.color_space == ShaderColorSpace::Unorm && format.is_srgb() {
            tracing::warn!(
                color_space = ?shader_config.color_space,
                surface_format = ?format,
                "non-srgb surface format was requested but unavailable; falling back to sRGB"
            );
        }
        tracing::info!(
            color_space = ?shader_config.color_space,
            surface_format = ?format,
            "shader runtime surface color format selected"
        );
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);
        let output_size = PhysicalSize::new(
            desktop_rect.width.max(1) as u32,
            desktop_rect.height.max(1) as u32,
        );
        let internal_size = compute_internal_render_size(output_size, shader_config.resolution);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: output_size.width,
            height: output_size.height,
            present_mode: self.config.present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: self.config.desired_maximum_frame_latency,
        };

        let estimated_bytes =
            estimate_swapchain_memory_bytes(config.width, config.height, SHADER_MAX_FRAME_LATENCY);
        let estimated_mb = bytes_to_megabytes(estimated_bytes);
        tracing::info!(
            output_width = output_size.width,
            output_height = output_size.height,
            internal_width = internal_size.width,
            internal_height = internal_size.height,
            resolution_percent = shader_config.resolution,
            desktop_scope = ?shader_config.desktop_scope,
            power_preference = ?SHADER_POWER_PREFERENCE,
            max_frame_latency = SHADER_MAX_FRAME_LATENCY,
            memory_target_mb = SHADER_MEMORY_TARGET_MB,
            estimated_swapchain_mb = estimated_mb,
            "shader runtime surface memory estimate"
        );
        if estimated_mb > SHADER_MEMORY_TARGET_MB as f64 {
            tracing::warn!(
                memory_target_mb = SHADER_MEMORY_TARGET_MB,
                estimated_swapchain_mb = estimated_mb,
                "shader swapchain estimate exceeds memory target; continuing shader mode"
            );
        }

        let (uniform_buffer, scene_bind_group, scene_pipeline) = create_scene_pipeline(
            &self.device,
            config.format,
            shader_bytes,
            shader_config.mouse_enabled,
            internal_size,
        )?;
        let composite_pipeline = create_composite_pipeline(
            &self.device,
            config.format,
            &self.composite_bind_group_layout,
        );
        let internal_target = create_internal_target(
            &self.device,
            internal_size,
            config.format,
            &self.composite_bind_group_layout,
            &self.composite_sampler,
        );

        self.surface.configure(&self.device, &config);
        self.config = config;
        self.scene_pipeline = scene_pipeline;
        self.scene_bind_group = scene_bind_group;
        self.uniform_buffer = uniform_buffer;
        self.composite_pipeline = composite_pipeline;
        self.internal_target = internal_target;
        self.mouse_enabled = shader_config.mouse_enabled;
        self.resolution_percent = shader_config.resolution;
        self.started_at = Instant::now();
        self.frame_index = 0;

        Ok(())
    }

    pub fn shutdown(self) {
        let started_at = Instant::now();
        tracing::info!("shader runtime shutdown started");

        let WgpuRuntime {
            _instance,
            surface,
            device,
            queue,
            config: _config,
            scene_pipeline,
            scene_bind_group,
            uniform_buffer,
            composite_pipeline,
            composite_bind_group_layout,
            composite_sampler,
            internal_target,
            started_at: _runtime_started_at,
            frame_index: _frame_index,
            mouse_enabled: _mouse_enabled,
            resolution_percent: _resolution_percent,
        } = self;

        poll_device_for_shutdown(&device);

        drop(internal_target);
        drop(scene_bind_group);
        drop(uniform_buffer);
        drop(scene_pipeline);
        drop(composite_pipeline);
        drop(composite_sampler);
        drop(composite_bind_group_layout);
        drop(queue);
        drop(surface);
        drop(device);
        drop(_instance);

        tracing::info!(
            elapsed_ms = started_at.elapsed().as_millis(),
            "shader runtime shutdown finished"
        );
    }

    pub fn render(&mut self, mouse: [f32; 2]) -> Result<()> {
        let output = match self.surface.get_current_texture() {
            Ok(output) => output,
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(wgpu::SurfaceError::Timeout) => {
                return Ok(());
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                bail!("GPU surface out of memory");
            }
        };

        let output_size = PhysicalSize::new(self.config.width, self.config.height);
        let scaled_mouse = scale_mouse_to_internal(mouse, output_size, self.internal_target.size);
        let uniforms = ShaderUniforms {
            time_seconds: self.started_at.elapsed().as_secs_f32(),
            frame_index: self.frame_index,
            mouse_enabled: u32::from(self.mouse_enabled),
            _padding: 0,
            resolution: [
                self.internal_target.size.width as f32,
                self.internal_target.size.height as f32,
                0.0,
                0.0,
            ],
            mouse: [scaled_mouse[0], scaled_mouse[1], 0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        self.frame_index = self.frame_index.wrapping_add(1);

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("aura-shader-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("aura-shader-scene-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.internal_target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.scene_pipeline);
            pass.set_bind_group(0, &self.scene_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("aura-shader-composite-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, &self.internal_target.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(())
    }

    fn recreate_internal_target(&mut self) {
        let output_size = PhysicalSize::new(self.config.width, self.config.height);
        let internal_size = compute_internal_render_size(output_size, self.resolution_percent);
        self.internal_target = create_internal_target(
            &self.device,
            internal_size,
            self.config.format,
            &self.composite_bind_group_layout,
            &self.composite_sampler,
        );
        tracing::info!(
            output_width = output_size.width,
            output_height = output_size.height,
            internal_width = internal_size.width,
            internal_height = internal_size.height,
            resolution_percent = self.resolution_percent,
            "shader runtime internal render target resized"
        );
    }
}

fn create_scene_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    shader_bytes: &[u8],
    mouse_enabled: bool,
    initial_resolution: PhysicalSize<u32>,
) -> Result<(wgpu::Buffer, wgpu::BindGroup, wgpu::RenderPipeline)> {
    let shader_words = load_spirv_words(shader_bytes)?;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("aura-live-shader"),
        source: wgpu::ShaderSource::SpirV(Cow::Owned(shader_words)),
    });

    let uniforms = ShaderUniforms {
        time_seconds: 0.0,
        frame_index: 0,
        mouse_enabled: u32::from(mouse_enabled),
        _padding: 0,
        resolution: [
            initial_resolution.width as f32,
            initial_resolution.height as f32,
            0.0,
            0.0,
        ],
        mouse: [0.0, 0.0, 0.0, 0.0],
    };

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("aura-shader-uniform-init"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("aura-shader-bind-group-layout"),
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
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("aura-shader-bind-group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("aura-shader-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("aura-shader-scene-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    Ok((uniform_buffer, bind_group, pipeline))
}

fn create_composite_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("aura-composite-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

fn create_composite_sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("aura-composite-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    })
}

fn create_composite_pipeline(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("aura-composite-shader"),
        source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(COMPOSITE_SHADER_SOURCE)),
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("aura-composite-pipeline-layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("aura-composite-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

fn create_internal_target(
    device: &wgpu::Device,
    size: PhysicalSize<u32>,
    format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
) -> InternalTarget {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("aura-shader-internal-target"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("aura-composite-bind-group"),
        layout: bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    InternalTarget {
        _texture: texture,
        view,
        bind_group,
        size,
    }
}

fn compute_internal_render_size(
    output_size: PhysicalSize<u32>,
    resolution_percent: u8,
) -> PhysicalSize<u32> {
    PhysicalSize::new(
        scale_dimension(output_size.width, resolution_percent),
        scale_dimension(output_size.height, resolution_percent),
    )
}

fn scale_dimension(dimension: u32, resolution_percent: u8) -> u32 {
    let scaled = (u64::from(dimension) * u64::from(resolution_percent)) / 100;
    scaled.max(1) as u32
}

fn scale_mouse_to_internal(
    mouse: [f32; 2],
    output_size: PhysicalSize<u32>,
    internal_size: PhysicalSize<u32>,
) -> [f32; 2] {
    if output_size.width == 0 || output_size.height == 0 {
        return mouse;
    }
    let x_scale = internal_size.width as f32 / output_size.width as f32;
    let y_scale = internal_size.height as f32 / output_size.height as f32;
    [mouse[0] * x_scale, mouse[1] * y_scale]
}

fn pick_surface_format(
    color_space: ShaderColorSpace,
    formats: &[wgpu::TextureFormat],
) -> wgpu::TextureFormat {
    match color_space {
        ShaderColorSpace::Unorm => pick_unorm_surface_format(formats),
        ShaderColorSpace::Srgb => formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(formats[0]),
    }
}

fn pick_unorm_surface_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    if let Some(format) = formats
        .iter()
        .copied()
        .find(|format| *format == wgpu::TextureFormat::Bgra8Unorm)
    {
        return format;
    }
    if let Some(format) = formats
        .iter()
        .copied()
        .find(|format| *format == wgpu::TextureFormat::Rgba8Unorm)
    {
        return format;
    }
    if let Some(format) = formats.iter().copied().find(|format| !format.is_srgb()) {
        return format;
    }
    formats[0]
}

fn load_spirv_words(bytes: &[u8]) -> Result<Vec<u32>> {
    if bytes.len() % 4 != 0 {
        bail!("embedded shader binary size is not a multiple of 4");
    }

    let mut words = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        words.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(words)
}

fn estimate_swapchain_memory_bytes(width: u32, height: u32, max_frame_latency: u8) -> u64 {
    let bytes_per_frame = u64::from(width) * u64::from(height) * 4;
    bytes_per_frame * (u64::from(max_frame_latency) + 1)
}

fn bytes_to_megabytes(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn poll_device_for_shutdown(device: &wgpu::Device) {
    let _ = device.poll(wgpu::Maintain::Wait);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_swapchain_memory_from_resolution_and_latency() {
        let bytes = estimate_swapchain_memory_bytes(3840, 2160, 1);
        assert_eq!(bytes, 66_355_200);
    }

    #[test]
    fn converts_bytes_to_megabytes() {
        let mb = bytes_to_megabytes(83_886_080);
        assert!((mb - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn computes_internal_render_size_from_percentage() {
        let output = PhysicalSize::new(1920, 1080);
        let scaled = compute_internal_render_size(output, 50);
        assert_eq!(scaled, PhysicalSize::new(960, 540));
    }

    #[test]
    fn keeps_full_size_at_hundred_percent() {
        let output = PhysicalSize::new(1920, 1080);
        let scaled = compute_internal_render_size(output, 100);
        assert_eq!(scaled, output);
    }

    #[test]
    fn internal_render_size_clamps_to_at_least_one_pixel() {
        let output = PhysicalSize::new(10, 10);
        let scaled = compute_internal_render_size(output, 1);
        assert_eq!(scaled, PhysicalSize::new(1, 1));
    }

    #[test]
    fn scales_mouse_coordinates_into_internal_space() {
        let output = PhysicalSize::new(1920, 1080);
        let internal = PhysicalSize::new(960, 540);
        let scaled = scale_mouse_to_internal([960.0, 540.0], output, internal);
        assert_eq!(scaled, [480.0, 270.0]);
    }

    #[test]
    fn picks_unorm_format_when_requested() {
        let formats = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm,
        ];
        let chosen = pick_surface_format(ShaderColorSpace::Unorm, &formats);
        assert_eq!(chosen, wgpu::TextureFormat::Bgra8Unorm);
    }

    #[test]
    fn picks_srgb_format_when_requested() {
        let formats = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let chosen = pick_surface_format(ShaderColorSpace::Srgb, &formats);
        assert_eq!(chosen, wgpu::TextureFormat::Rgba8UnormSrgb);
    }

    #[test]
    fn falls_back_when_unorm_is_unavailable() {
        let formats = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ];
        let chosen = pick_surface_format(ShaderColorSpace::Unorm, &formats);
        assert_eq!(chosen, wgpu::TextureFormat::Bgra8UnormSrgb);
    }
}
