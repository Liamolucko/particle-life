use std::f32::consts::TAU;
use std::mem::size_of;
use std::num::NonZeroU64;
use std::time::Duration;

use bytemuck::Pod;
use bytemuck::Zeroable;
use glam::vec2;
use glam::vec4;
use glam::Vec2;
use glam::Vec4;
use instant::Instant;
use palette::LinSrgb;
use rand::rngs::OsRng;
use rand::Rng;
use sim::Sim;
use sim::RADIUS;
use wgpu::include_wgsl;
use wgpu::util::BufferInitDescriptor;
use wgpu::util::DeviceExt;
use wgpu::Backends;
use wgpu::BindGroup;
use wgpu::BindGroupDescriptor;
use wgpu::BindGroupEntry;
use wgpu::BindGroupLayoutDescriptor;
use wgpu::BindGroupLayoutEntry;
use wgpu::BindingResource;
use wgpu::BindingType;
use wgpu::BlendComponent;
use wgpu::BlendFactor;
use wgpu::BlendOperation;
use wgpu::BlendState;
use wgpu::Buffer;
use wgpu::BufferBinding;
use wgpu::BufferBindingType;
use wgpu::BufferUsages;
use wgpu::ColorTargetState;
use wgpu::ColorWrites;
use wgpu::CommandEncoderDescriptor;
use wgpu::Device;
use wgpu::FragmentState;
use wgpu::Limits;
use wgpu::MultisampleState;
use wgpu::PipelineLayoutDescriptor;
use wgpu::PresentMode;
use wgpu::PrimitiveState;
use wgpu::Queue;
use wgpu::RenderPipeline;
use wgpu::RenderPipelineDescriptor;
use wgpu::RequestAdapterOptions;
use wgpu::ShaderStages;
use wgpu::Surface;
use wgpu::SurfaceConfiguration;
use wgpu::TextureDescriptor;
use wgpu::TextureDimension;
use wgpu::TextureFormat;
use wgpu::TextureUsages;
use wgpu::TextureView;
use wgpu::TextureViewDescriptor;
use wgpu::VertexBufferLayout;
use wgpu::VertexState;
use wgpu::VertexStepMode;
use winit::dpi::LogicalSize;
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub mod settings;
pub mod sim;

use settings::Settings;

const CIRCLE_POINTS: usize = 32;
const SAMPLE_COUNT: u32 = 4;
const MAX_PARTICLES: usize = 600;
const PARTICLE_SEGMENT_SIZE: u64 = (size_of::<GpuParticle>() * MAX_PARTICLES) as u64;

/// The number of past frames to use to create trails behind each particle.
const TRAIL_LENGTH: u64 = 10;

// The particle information sent to the GPU.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy, Zeroable, Pod)]
pub struct GpuParticle {
    pos: Vec2,
    color: LinSrgb,
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug)]
pub struct RenderSettings {
    pub wrap: u32,

    pub zoom: f32,
    pub camera: Vec2,

    pub padding: [u32; 2],

    /// The horizontal/vertical radius of a particle in clip space.
    /// (A perfect circle in pixel space isn't always a perfect circle in clip space, hence why can't just pass `radius`.)
    pub horiz_rad: f32,
    pub vert_rad: f32,

    // stupid webgl alignment stuff means that vec2s in arrays are basically treated as vec4s.
    pub circle_points: [Vec4; CIRCLE_POINTS],
}

impl RenderSettings {
    /// Create a `RenderSettings` with all the size information set for the given `size`,
    /// and all the state information set to default.
    pub fn new(size: LogicalSize<f32>) -> Self {
        RenderSettings {
            wrap: 0,

            zoom: 1.0,
            camera: vec2(0.0, 0.0),

            padding: [0; 2],

            horiz_rad: 2.0 * RADIUS / size.width,
            vert_rad: 2.0 * RADIUS / size.height,

            circle_points: circle_points(size),
        }
    }
}

fn create_multisampled_framebuffer(
    device: &Device,
    format: TextureFormat,
    width: u32,
    height: u32,
) -> TextureView {
    device
        .create_texture(&TextureDescriptor {
            label: Some("Multisampled framebuffer"),
            size: wgpu::Extent3d {
                width,
                height,
                ..Default::default()
            },
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT,
        })
        .create_view(&TextureViewDescriptor::default())
}

fn opacities() -> impl Iterator<Item = f32> {
    (1..=TRAIL_LENGTH).map(|n| n as f32 / TRAIL_LENGTH as f32)
}

fn circle_points(size: LogicalSize<f32>) -> [Vec4; CIRCLE_POINTS] {
    let mut out = [Vec4::ZERO; CIRCLE_POINTS];

    for (i, point) in out.iter_mut().enumerate() {
        let angle = TAU * (i as f32) / (CIRCLE_POINTS as f32);
        *point = vec4(
            2.0 * RADIUS * angle.cos() / size.width,
            2.0 * RADIUS * angle.sin() / size.height,
            0.0,
            0.0,
        );
    }

    out
}

pub struct State {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface,

    pub settings_buffer: Buffer,
    pub particle_buffer: Buffer,

    pub settings_bind_group: BindGroup,
    pub opacity_bind_groups: Vec<BindGroup>,

    pub render_pipeline: RenderPipeline,

    pub swapchain_format: TextureFormat,
    pub multisampled_framebuffer: TextureView,

    pub last_step: Instant,
    /// The index of the next segment of the particle buffer to be written to.
    pub particle_segment: u64,
    pub step_rate: u32,

    pub sim: Sim,

    // It's easier to keep track of these externally than read them from GPU memory every time.
    pub zoom: f32,
    pub camera: Vec2,
}

impl State {
    pub async fn new(window: &Window) -> Self {
        let settings = Settings::balanced();

        let instance = wgpu::Instance::new(Backends::all());

        let surface = unsafe { instance.create_surface(window) };

        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                // Make sure this adapter can render to the window.
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    limits: Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to obtain device");

        let mut rng = OsRng;

        let size = window.inner_size();
        let logical_size = size.to_logical(window.scale_factor());

        let render_settings = RenderSettings::new(logical_size);

        let settings_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Settings buffer"),
            contents: bytemuck::bytes_of(&render_settings),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let sim = Sim::new(settings, &mut rng);

        let particles = sim.export_particles();

        let particle_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Particle buffer"),
            contents: bytemuck::cast_slice(&[particles; TRAIL_LENGTH as usize]),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let opacity_buffers: Vec<_> = opacities()
            .map(|opacity| {
                device.create_buffer_init(&BufferInitDescriptor {
                    label: Some(&format!("{} opacity buffer", opacity)),
                    contents: bytemuck::cast_slice(&[opacity, 0.0, 0.0, 0.0]),
                    usage: BufferUsages::UNIFORM,
                })
            })
            .collect();

        let settings_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Settings bind group layout"),
                entries: &[
                    // settings
                    BindGroupLayoutEntry {
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(size_of::<RenderSettings>() as u64),
                        },
                        binding: 0,
                        visibility: ShaderStages::VERTEX,
                        count: None,
                    },
                ],
            });

        let settings_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Settings bind group"),
            layout: &settings_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &settings_buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let opacity_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Opacity bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(4),
                    },
                    count: None,
                }],
            });

        let opacity_bind_groups: Vec<_> = opacities()
            .enumerate()
            .map(|(i, opacity)| {
                device.create_bind_group(&BindGroupDescriptor {
                    label: Some(&format!("{} opacity bind group", opacity)),
                    layout: &opacity_bind_group_layout,
                    entries: &[BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &opacity_buffers[i],
                            offset: 0,
                            size: None,
                        }),
                    }],
                })
            })
            .collect();

        let swapchain_format = surface.get_preferred_format(&adapter).unwrap();

        let shader = device.create_shader_module(&include_wgsl!("shader.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            bind_group_layouts: &[&settings_bind_group_layout, &opacity_bind_group_layout],
            ..Default::default()
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[
                    // Particle buffer
                    VertexBufferLayout {
                        array_stride: size_of::<GpuParticle>() as u64,
                        step_mode: VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
                    },
                ],
            },
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState {
                count: SAMPLE_COUNT,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[ColorTargetState {
                    format: swapchain_format,
                    // some basic blending, to make the translucent trails work.
                    // I don't really know what I'm doing when it comes to this, but this works ok.
                    blend: Some(BlendState {
                        color: BlendComponent {
                            src_factor: BlendFactor::SrcAlpha,
                            dst_factor: BlendFactor::OneMinusSrcAlpha,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            src_factor: BlendFactor::One,
                            dst_factor: BlendFactor::One,
                            operation: BlendOperation::Add,
                        },
                    }),
                    write_mask: ColorWrites::ALL,
                }],
            }),
            multiview: None,
        });

        let multisampled_framebuffer =
            create_multisampled_framebuffer(&device, swapchain_format, size.width, size.height);

        surface.configure(
            &device,
            &SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: swapchain_format,
                width: size.width,
                height: size.height,
                present_mode: PresentMode::Fifo,
            },
        );

        Self {
            device,
            queue,
            surface,

            settings_buffer,
            particle_buffer,

            settings_bind_group,
            opacity_bind_groups,

            render_pipeline,

            swapchain_format,
            multisampled_framebuffer,

            last_step: Instant::now(),
            particle_segment: 0,
            step_rate: 300,

            sim,

            zoom: 1.0,
            camera: vec2(0.0, 0.0),
        }
    }

    pub fn resize(&mut self, size: PhysicalSize<u32>, scale_factor: f64) {
        self.surface.configure(
            &self.device,
            &SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: self.swapchain_format,
                width: size.width,
                height: size.height,
                present_mode: PresentMode::Fifo,
            },
        );

        // Replace the framebuffer with a new one the correct size
        self.multisampled_framebuffer = create_multisampled_framebuffer(
            &self.device,
            self.swapchain_format,
            size.width,
            size.height,
        );

        let logical_size: LogicalSize<f32> = size.to_logical(scale_factor);

        let new_settings = RenderSettings::new(logical_size);

        // Update the resolution & circle points in `RenderSettings`.
        self.queue.write_buffer(
            &self.settings_buffer,
            24,
            &bytemuck::bytes_of(&new_settings)[24..],
        );
    }

    pub fn render(&mut self, width: f32, height: f32) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");

        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        let step_period = Duration::from_secs(1) / self.step_rate;
        let mut steps = 0;
        while self.last_step + step_period < Instant::now() {
            self.last_step += step_period;

            self.sim.step(width, height);

            self.particle_segment += 1;
            self.particle_segment %= TRAIL_LENGTH;

            let offset = self.particle_segment * PARTICLE_SEGMENT_SIZE;

            self.queue.write_buffer(
                &self.particle_buffer,
                offset,
                bytemuck::cast_slice(&self.sim.export_particles()),
            );

            steps += 1;

            if steps == 20 {
                // It's not worth trying to catch up that far, just reset from here.
                self.last_step = Instant::now();
            }
        }

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.multisampled_framebuffer,
                    resolve_target: Some(&view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: false,
                    },
                }],
                depth_stencil_attachment: None,
            });
            rpass.set_pipeline(&self.render_pipeline);

            rpass.set_bind_group(0, &self.settings_bind_group, &[]);

            for (j, i) in (self.particle_segment + 1..)
                .map(|i| i % TRAIL_LENGTH)
                .take(TRAIL_LENGTH as usize)
                .enumerate()
            {
                let offset = i * PARTICLE_SEGMENT_SIZE;
                rpass.set_vertex_buffer(
                    0,
                    self.particle_buffer.slice(
                        offset
                            ..offset + (self.sim.particles.len() * size_of::<GpuParticle>()) as u64,
                    ),
                );
                rpass.set_bind_group(1, &self.opacity_bind_groups[j], &[]);
                rpass.draw(
                    0..CIRCLE_POINTS as u32 * 3,
                    0..self.sim.particles.len() as u32,
                );
            }
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();
    }

    pub fn toggle_wrap(&mut self) {
        self.sim.wrap = !self.sim.wrap;

        self.queue.write_buffer(
            &self.settings_buffer,
            0,
            bytemuck::bytes_of(&(self.sim.wrap as u32)),
        );

        // Make sure the camera is within bounds
        self.set_camera();
    }

    pub fn replace_settings<R: Rng>(&mut self, settings: Settings, rng: &mut R) {
        self.sim = Sim {
            wrap: self.sim.wrap,
            ..Sim::new(settings, rng)
        };

        self.regenerate_particles(rng);
    }

    pub fn regenerate_particles<R: Rng>(&mut self, rng: &mut R) {
        self.sim.regenerate_particles(rng);

        // Reset camera and zoom
        self.camera = vec2(0.0, 0.0);
        self.zoom = 1.0;
        self.set_camera();
    }

    /// Sets the camera zoom and position.
    pub fn set_camera(&mut self) {
        if !self.sim.wrap {
            let view_radius = 1.0 / self.zoom;

            self.camera = self.camera.clamp(
                vec2(-1.0 + view_radius, -1.0 + view_radius),
                vec2(1.0 - view_radius, 1.0 - view_radius),
            );
        } else {
            while self.camera[0] > 1.0 {
                self.camera[0] -= 2.0;
            }

            while self.camera[0] < -1.0 {
                self.camera[0] += 2.0;
            }

            while self.camera[1] > 1.0 {
                self.camera[1] -= 2.0;
            }

            while self.camera[1] < -1.0 {
                self.camera[1] += 2.0;
            }
        }

        self.queue.write_buffer(
            &self.settings_buffer,
            4,
            bytemuck::bytes_of(&[self.zoom, self.camera[0], self.camera[1]]),
        )
    }
}
