use std::f32::consts::TAU;
use std::mem::size_of;
use std::num::NonZeroU64;
use std::time::Duration;

use bytemuck::Pod;
use bytemuck::Zeroable;
use instant::Instant;
use rand::rngs::OsRng;
use rand::Rng;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;
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
use wgpu::ComputePassDescriptor;
use wgpu::ComputePipeline;
use wgpu::ComputePipelineDescriptor;
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
use winit::window::Window;

pub mod settings;

use settings::RuntimeSettings;
use settings::Settings;

const RADIUS: f32 = 10.0;
const DIAMETER: f32 = RADIUS * 2.0;

const CIRCLE_POINTS: usize = 24;
const SAMPLE_COUNT: u32 = 4;

/// The number of past frames to use to create trails behind each particle.
const TRAIL_LENGTH: usize = 10;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct Particle {
    pos: [f32; 2],
    vel: [f32; 2],
    kind: u32,
    padding: [u8; 4],
}

/// Returns some points around the edge of a circle, as well as a 0 at the end to use for the centre.
fn circle_points(win_width: u32, win_height: u32) -> [[f32; 2]; CIRCLE_POINTS + 1] {
    // We want 10px radius particles, so figure out what that maps to in clip space with this resolution.
    let x_size = RADIUS / win_width as f32;
    let y_size = RADIUS / win_height as f32;

    let mut out = [[0.0; 2]; CIRCLE_POINTS + 1];
    for i in 0..CIRCLE_POINTS {
        let angle = (TAU / CIRCLE_POINTS as f32) * i as f32;
        out[i][0] = angle.cos() * x_size;
        out[i][1] = angle.sin() * y_size;
    }
    out
}

/// Generates the needed indices into `circle_points`.
fn circle_indices() -> [[u16; 3]; CIRCLE_POINTS + 1] {
    let mut triangles = [[0; 3]; CIRCLE_POINTS + 1];
    for i in 0..CIRCLE_POINTS {
        triangles[i] = [
            CIRCLE_POINTS as u16,
            i as u16,
            (i as u16 + 1) % CIRCLE_POINTS as u16,
        ];
    }
    triangles
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

fn generate_particles<R: Rng>(settings: Settings, rng: &mut R) -> Vec<Particle> {
    let kinds = Uniform::new(0, settings.kinds as u32);
    // This is in clip space, so it ranges from -1 to 1.
    let pos_dist = Uniform::new(-0.5, 0.5);
    let vel_dist = Normal::new(0.0, 0.2).unwrap();

    // Always generate 600 particles so we don't have to worry about resizing the particle buffers.
    // The buffers will be sliced so that only the correct amount are actually used.
    let mut particles = Vec::with_capacity(600);
    for _ in 0..600 {
        particles.push(Particle {
            kind: kinds.sample(rng),
            pos: [pos_dist.sample(rng), pos_dist.sample(rng)],
            vel: [vel_dist.sample(rng), vel_dist.sample(rng)],

            padding: [0; 4],
        })
    }
    particles
}

fn opacities() -> impl Iterator<Item = f32> {
    (1..=TRAIL_LENGTH).map(|n| n as f32 / TRAIL_LENGTH as f32)
}

pub struct State {
    pub device: Device,
    pub queue: Queue,
    pub surface: Surface,

    pub settings_buffer: Buffer,
    pub particle_buffers: Vec<Buffer>,

    pub circle_vertex_buffer: Buffer,
    pub circle_index_buffer: Buffer,

    pub settings_bind_group: BindGroup,
    pub particle_bind_groups: Vec<BindGroup>,
    pub opacity_bind_groups: Vec<BindGroup>,

    pub compute_pipeline: ComputePipeline,
    pub render_pipeline: RenderPipeline,

    pub swapchain_format: TextureFormat,
    pub multisampled_framebuffer: TextureView,

    pub settings: Settings,

    pub last_frame: Instant,
    pub step_number: usize,
    pub step_rate: u32,

    // It's easier to keep track of these externally than read them from GPU memory every time.
    pub wrap: bool,
    pub zoom: f32,
    pub camera: [f32; 2],
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
                    limits: Limits::downlevel_defaults().using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to obtain device");

        let mut rng = OsRng;

        let size = window.inner_size();

        let runtime_settings =
            RuntimeSettings::generate(settings, size.width, size.height, &mut rng);

        let settings_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Settings buffer"),
            contents: bytemuck::bytes_of(&runtime_settings),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        let particles = generate_particles(settings, &mut rng);

        let particle_buffers: Vec<_> = (1..=TRAIL_LENGTH + 1)
            .map(|n| {
                device.create_buffer_init(&BufferInitDescriptor {
                    label: Some(&format!("Particle buffer {}", n)),
                    contents: bytemuck::cast_slice(&particles),
                    usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::COPY_DST,
                })
            })
            .collect();

        let circle_vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Circle vertex buffer"),
            contents: bytemuck::cast_slice(&circle_points(size.width, size.height)),
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        let circle_index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Circle index buffer"),
            contents: bytemuck::cast_slice(&circle_indices()),
            usage: BufferUsages::INDEX,
        });

        let opacity_buffers: Vec<_> = opacities()
            .map(|opacity| {
                device.create_buffer_init(&BufferInitDescriptor {
                    label: Some(&format!("{} opacity buffer", opacity)),
                    contents: bytemuck::bytes_of(&opacity),
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
                            min_binding_size: NonZeroU64::new(size_of::<RuntimeSettings>() as u64),
                        },
                        binding: 0,
                        visibility: ShaderStages::VERTEX | ShaderStages::COMPUTE,
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

        let particle_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    // in_particles
                    BindGroupLayoutEntry {
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(size_of::<Particle>() as u64),
                        },
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        count: None,
                    },
                    // out_particles
                    BindGroupLayoutEntry {
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: NonZeroU64::new(size_of::<Particle>() as u64),
                        },
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        count: None,
                    },
                ],
            });

        let particle_bind_groups: Vec<_> = particle_buffers
            .iter()
            .zip(
                // Offset the second buffer by 1, and use `cycle` to have the last one wrap back to the start.
                particle_buffers
                    .iter()
                    .cycle()
                    .skip(1)
                    .take(particle_buffers.len()),
            )
            .enumerate()
            .map(|(i, (in_buf, out_buf))| {
                device.create_bind_group(&BindGroupDescriptor {
                    label: Some(&format!("Particle buffer {}", i + 1)),
                    layout: &particle_bind_group_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: BindingResource::Buffer(BufferBinding {
                                buffer: in_buf,
                                offset: 0,
                                size: None,
                            }),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::Buffer(BufferBinding {
                                buffer: out_buf,
                                offset: 0,
                                size: None,
                            }),
                        },
                    ],
                })
            })
            .collect();

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

        let compute_shader = device.create_shader_module(&include_wgsl!("compute.wgsl"));

        let compute_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            bind_group_layouts: &[&settings_bind_group_layout, &particle_bind_group_layout],
            ..Default::default()
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Physics compute pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: "update_velocity",
        });

        let swapchain_format = surface.get_preferred_format(&adapter).unwrap();

        let render_shader = device.create_shader_module(&include_wgsl!("render.wgsl"));

        let render_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            bind_group_layouts: &[&settings_bind_group_layout, &opacity_bind_group_layout],
            ..Default::default()
        });

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Render pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: VertexState {
                module: &render_shader,
                entry_point: "vs_main",
                buffers: &[
                    // Particle buffer
                    VertexBufferLayout {
                        array_stride: size_of::<Particle>() as u64,
                        step_mode: VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32],
                    },
                    // Circle vertex buffer
                    VertexBufferLayout {
                        array_stride: 8,
                        step_mode: VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![3 => Float32x2],
                    }
                ]
            },
            primitive: PrimitiveState ::default(),
            depth_stencil: None,
            multisample: MultisampleState {
                count: SAMPLE_COUNT,
                ..Default::default()
            },
            fragment: Some(FragmentState {
                module: &render_shader,
                entry_point: "fs_main",
                targets: &[ColorTargetState {
                    format: swapchain_format.into(),
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
                }]
            }),
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
            particle_buffers,

            circle_vertex_buffer,
            circle_index_buffer,

            settings_bind_group,
            particle_bind_groups,
            opacity_bind_groups,

            compute_pipeline,
            render_pipeline,

            swapchain_format,
            multisampled_framebuffer,

            settings,

            last_frame: Instant::now(),
            step_number: 0,
            step_rate: 300,

            wrap: false,
            zoom: 1.0,
            camera: [0.0, 0.0],
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface.configure(
            &self.device,
            &SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: self.swapchain_format,
                width,
                height,
                present_mode: PresentMode::Fifo,
            },
        );

        // Replace the framebuffer with a new one the correct size
        self.multisampled_framebuffer =
            create_multisampled_framebuffer(&self.device, self.swapchain_format, width, height);

        // Update the resolution in `RuntimeSettings`, which is at offset 0.
        self.queue.write_buffer(
            &self.settings_buffer,
            0,
            bytemuck::bytes_of(&[width as f32, height as f32]),
        );

        // Update the circle points.
        self.queue.write_buffer(
            &self.circle_vertex_buffer,
            0,
            bytemuck::bytes_of(&circle_points(width, height)),
        );
    }

    pub fn render(&mut self) {
        let frame = self
            .surface
            .get_current_frame()
            .expect("Failed to acquire next swap chain texture")
            .output;

        let view = frame.texture.create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor { label: None });

        {
            let mut cpass = encoder.begin_compute_pass(&ComputePassDescriptor { label: None });
            cpass.set_pipeline(&self.compute_pipeline);
            cpass.set_bind_group(0, &self.settings_bind_group, &[]);

            let step_period = Duration::from_secs(1) / self.step_rate;
            let mut steps = 0;
            while self.last_frame + step_period < Instant::now() {
                self.last_frame += step_period;
                self.step_number += 1;
                self.step_number %= TRAIL_LENGTH + 1;

                cpass.set_bind_group(1, &self.particle_bind_groups[self.step_number], &[]);
                cpass.dispatch(self.settings.particles as u32 / 100, 1, 1);

                steps += 1;

                if steps == 20 {
                    // It's not worth trying to catch up that far, just reset from here.
                    self.last_frame = Instant::now();
                }
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
            rpass.set_vertex_buffer(1, self.circle_vertex_buffer.slice(..));
            rpass.set_index_buffer(
                self.circle_index_buffer.slice(..),
                wgpu::IndexFormat::Uint16,
            );

            for (j, i) in (self.step_number + 2..)
                .map(|i| i % (TRAIL_LENGTH + 1))
                .take(TRAIL_LENGTH)
                .enumerate()
            {
                rpass.set_vertex_buffer(0, self.particle_buffers[i].slice(..(self.settings.particles * size_of::<Particle>()) as u64));
                rpass.set_bind_group(1, &self.opacity_bind_groups[j], &[]);
                rpass.draw_indexed(
                    0..CIRCLE_POINTS as u32 * 3,
                    0,
                    0..self.settings.particles as u32,
                );
            }
        }

        self.queue.submit(Some(encoder.finish()));
    }

    pub fn toggle_wrap(&mut self) {
        self.wrap = !self.wrap;

        let flags = (self.wrap as u32) << 1 | self.settings.flat_force as u32;
        self.queue
            .write_buffer(&self.settings_buffer, 12, bytemuck::bytes_of(&flags))
    }

    pub fn replace_settings<R: Rng>(&mut self, settings: Settings, rng: &mut R) {
        self.settings = settings;

        self.regenerate_kinds(rng);

        self.regenerate_particles(rng);
    }

    pub fn regenerate_particles<R: Rng>(&mut self, rng: &mut R) {
        let particles = generate_particles(self.settings, rng);

        for buffer in self.particle_buffers.iter() {
            self.queue
                .write_buffer(&buffer, 0, bytemuck::cast_slice(&particles));
        }

        // Reset camera and zoom
        self.camera = [0.0, 0.0];
        self.zoom = 1.0;
        self.set_camera();
    }

    fn regenerate_kinds<R: Rng>(&mut self, rng: &mut R) {
        // Use dummy width and height, and then just leave the existing values there by only updating the latter part.
        let mut runtime_settings = RuntimeSettings::generate(self.settings, 0, 0, rng);

        // Set the wrap flag correctly
        runtime_settings.flags |= (self.wrap as u32) << 1;

        self.queue.write_buffer(
            &self.settings_buffer,
            8,
            &bytemuck::bytes_of(&runtime_settings)[8..],
        );
    }

    /// Sets the camera zoom and position.
    pub fn set_camera(&mut self) {
        if !self.wrap {
            let view_radius = 1.0 / self.zoom;

            self.camera = [
                self.camera[0].clamp(-1.0 + view_radius, 1.0 - view_radius),
                self.camera[1].clamp(-1.0 + view_radius, 1.0 - view_radius),
            ]
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

        self.queue.write_buffer(&self.settings_buffer, size_of::<RuntimeSettings>() as u64 - 16, bytemuck::bytes_of(&[self.camera[0], self.camera[1], self.zoom]))
    }
}
