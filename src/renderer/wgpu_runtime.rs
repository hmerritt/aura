use crate::errors::Result;
use anyhow::{anyhow, bail, Context};
use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

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

pub struct WgpuRuntime {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    started_at: Instant,
    frame_index: u32,
    mouse_enabled: bool,
}

impl WgpuRuntime {
    pub fn new(window: Arc<Window>, shader_bytes: &[u8], mouse_enabled: bool) -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surface = instance
            .create_surface(window.clone())
            .map_err(|error| anyhow!("failed to create wgpu surface: {error}"))?;

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .ok_or_else(|| anyhow!("failed to find a compatible GPU adapter"))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("bgm-shader-device"),
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
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);

        let size = window.inner_size();
        let mut config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let (uniform_buffer, bind_group, pipeline) =
            create_pipeline(&device, &config, shader_bytes, mouse_enabled)?;

        config.width = size.width.max(1);
        config.height = size.height.max(1);

        Ok(Self {
            _instance: instance,
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            uniform_buffer,
            started_at: Instant::now(),
            frame_index: 0,
            mouse_enabled,
        })
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);
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

        let uniforms = ShaderUniforms {
            time_seconds: self.started_at.elapsed().as_secs_f32(),
            frame_index: self.frame_index,
            mouse_enabled: u32::from(self.mouse_enabled),
            _padding: 0,
            resolution: [
                self.config.width as f32,
                self.config.height as f32,
                0.0,
                0.0,
            ],
            mouse: [mouse[0], mouse[1], 0.0, 0.0],
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
                label: Some("bgm-shader-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bgm-shader-pass"),
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit([encoder.finish()]);
        output.present();
        Ok(())
    }
}

fn create_pipeline(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    shader_bytes: &[u8],
    mouse_enabled: bool,
) -> Result<(wgpu::Buffer, wgpu::BindGroup, wgpu::RenderPipeline)> {
    let shader_words = load_spirv_words(shader_bytes)?;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("bgm-live-shader"),
        source: wgpu::ShaderSource::SpirV(Cow::Owned(shader_words)),
    });

    let uniforms = ShaderUniforms {
        time_seconds: 0.0,
        frame_index: 0,
        mouse_enabled: u32::from(mouse_enabled),
        _padding: 0,
        resolution: [config.width as f32, config.height as f32, 0.0, 0.0],
        mouse: [0.0, 0.0, 0.0, 0.0],
    };

    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("bgm-shader-uniform-init"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("bgm-shader-bind-group-layout"),
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
        label: Some("bgm-shader-bind-group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("bgm-shader-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("bgm-shader-pipeline"),
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
                format: config.format,
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
