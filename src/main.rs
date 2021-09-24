use std::f32::consts::TAU;
use std::mem::size_of;
use std::num::NonZeroU64;
use std::sync::Arc;

use bytemuck::Pod;
use bytemuck::Zeroable;
use palette::Hsv;
use palette::IntoColor;
use palette::Srgb;
use rand::rngs::OsRng;
use rand::Rng;
use rand_distr::Distribution;
use rand_distr::Normal;
use rand_distr::Uniform;
use wgpu::include_wgsl;
use wgpu::util::BufferInitDescriptor;
use wgpu::util::DeviceExt;
use wgpu::BindGroupDescriptor;
use wgpu::BindGroupEntry;
use wgpu::BindGroupLayoutDescriptor;
use wgpu::BindGroupLayoutEntry;
use wgpu::BindingResource;
use wgpu::BindingType;
use wgpu::BufferBinding;
use wgpu::BufferBindingType;
use wgpu::BufferUsages;
use wgpu::ComputePassDescriptor;
use wgpu::ComputePipelineDescriptor;
use wgpu::Limits;
use wgpu::Maintain;
use wgpu::PipelineLayoutDescriptor;
use wgpu::ShaderStages;
use wgpu::TextureUsages;
use winit::event::Event;
use winit::event::WindowEvent;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::window::Window;

#[cfg(not(target_arch = "wasm32"))]
use tokio::spawn;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local as spawn;

use crate::settings::Settings;

mod settings;

const RADIUS: f32 = 10.0;
const DIAMETER: f32 = RADIUS * 2.0;
const CIRCLE_POINTS: usize = 24;
const MULTISAMPLE_COUNT: u32 = 4;

/// The number of kinds of particles which are always generated.
/// The `kinds` field of `Settings` then just specifies which are actually used.
const KINDS: usize = 20;

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
    // We want 20px radius particles, so figure out what that maps to in clip space with this resolution.
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
fn indices() -> [[u16; 3]; CIRCLE_POINTS + 1] {
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

fn gen_random_particles<R: Rng>(settings: Settings, rng: &mut R) -> Vec<Particle> {
    let kinds = Uniform::new(0, settings.kinds as u32);
    // This is in clip space, so it ranges from -1 to 1.
    let pos_dist = Uniform::new(-0.5, 0.5);
    let vel_dist = Normal::new(0.0, 0.2).unwrap();

    let mut particles = Vec::with_capacity(settings.particles);
    for _ in 0..settings.particles {
        particles.push(Particle {
            kind: kinds.sample(rng),
            pos: [pos_dist.sample(rng), pos_dist.sample(rng)],
            vel: [vel_dist.sample(rng), vel_dist.sample(rng)],

            padding: [0; 4],
        })
    }
    particles
}

/// The symmetric properties of two kinds of particles.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SymmetricProperties {
    /// The distance below which particles begin to unconditionally repel each other.
    pub repel_distance: f32,
    /// The distance above which particles have no influence on each other.
    pub influence_radius: f32,
}

// This is used instead of a plain [f32; 3] so that we can give it an alignment of 16, which vec3 has for some reason.
#[repr(C, align(16))]
#[derive(Pod, Zeroable, Clone, Copy, Default)]
pub struct Color {
    red: f32,
    green: f32,
    blue: f32,
    // make an actual field for this padding, so that it's accepted by bytemuck.
    padding: [u8; 4],
}

impl From<Srgb> for Color {
    fn from(color: Srgb) -> Self {
        Self {
            red: color.red,
            green: color.green,
            blue: color.blue,
            padding: [0; 4],
        }
    }
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
struct RuntimeSettings {
    friction: f32,
    // actually a bool, kinda
    flat_force: u32,

    width: f32,
    height: f32,

    colors: [Color; KINDS],
    symmetric_props: [SymmetricProperties; KINDS * (KINDS + 1) / 2],
    attractions: [f32; KINDS * KINDS],
}

impl RuntimeSettings {
    fn generate<R: Rng>(settings: Settings, width: u32, height: u32, rng: &mut R) -> Self {
        let mut this = Self {
            friction: settings.friction,
            flat_force: settings.flat_force as u32,

            width: width as f32,
            height: height as f32,

            colors: [Color::default(); KINDS],
            symmetric_props: [SymmetricProperties {
                influence_radius: 0.0,
                repel_distance: 0.0,
            }; KINDS * (KINDS + 1) / 2],
            attractions: [0.0; KINDS * KINDS],
        };

        // The angle between each color's hue.
        let angle = 360.0 / KINDS as f32;

        for i in 0..KINDS {
            let value = if i % 2 == 0 { 0.5 } else { 1.0 };
            let color: Srgb = Hsv::new(angle * i as f32, 1.0, value).into_color();
            // this last element isn't alpha; it's just padding.
            this.colors[i] = color.into();

            for j in 0..KINDS {
                let index = i * KINDS + j;
                this.attractions[index] = if i == j {
                    -f32::abs(settings.attraction_distr.sample(rng))
                } else {
                    settings.attraction_distr.sample(rng)
                };

                if j <= i {
                    let repel_distance = if i == j {
                        DIAMETER
                    } else {
                        settings.repel_distance_distr.sample(rng)
                    };
                    let mut influence_radius = settings.influence_radius_distr.sample(rng);
                    if influence_radius < repel_distance {
                        influence_radius = repel_distance;
                    }

                    let index = i * (i + 1) / 2 + j;

                    this.symmetric_props[index] = SymmetricProperties {
                        repel_distance,
                        influence_radius,
                    };
                }
            }
        }

        this
    }
}

async fn run(event_loop: EventLoop<()>, window: Window) {
    let settings = Settings::balanced();

    let size = window.inner_size();
    let instance = wgpu::Instance::new(wgpu::Backends::all() & !wgpu::Backends::BROWSER_WEBGPU);
    let surface = unsafe { instance.create_surface(&window) };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            // Request an adapter which can render to our surface
            compatible_surface: Some(&surface),
        })
        .await
        .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
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
        .expect("Failed to create device");

    // Load the shaders from disk
    let shader = device.create_shader_module(&include_wgsl!("shader.wgsl"));

    let swapchain_format = surface.get_preferred_format(&adapter).unwrap();

    let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: None,
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
            // particles
            BindGroupLayoutEntry {
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<Particle>() as u64),
                },
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                count: None,
            },
            // back_particles
            BindGroupLayoutEntry {
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<Particle>() as u64),
                },
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        bind_group_layouts: &[&bind_group_layout],
        ..Default::default()
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[
                // The position + color of each particle
                wgpu::VertexBufferLayout {
                    array_stride: size_of::<Particle>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32],
                },
                // The circle points
                wgpu::VertexBufferLayout {
                    array_stride: size_of::<[f32; 2]>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![3 => Float32x2],
                },
            ],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[swapchain_format.into()],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: MULTISAMPLE_COUNT,
            ..Default::default()
        },
    });

    let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        entry_point: "update_velocity",
        module: &shader,
    });

    let circle_points_buffer = device.create_buffer_init(
        &wgpu::util::BufferInitDescriptor {
            label: Some("Circle points buffer"),
            contents: bytemuck::bytes_of(&circle_points(size.width, size.height)),
            usage: BufferUsages::VERTEX | BufferUsages::MAP_WRITE,
        },
    );

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Circle points index buffer"),
        contents: bytemuck::bytes_of(&indices()),
        usage: BufferUsages::INDEX,
    });

    let particles = gen_random_particles(settings, &mut OsRng);

    let particle_buffers = Arc::new([
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle buffer 1"),
            contents: bytemuck::cast_slice(&particles),
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::MAP_WRITE,
        }),
        // Initialize the second buffer as well so that the kinds are correct.
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle buffer 2"),
            contents: bytemuck::cast_slice(&particles),
            usage: BufferUsages::VERTEX | BufferUsages::STORAGE | BufferUsages::MAP_WRITE,
        }),
    ]);

    let runtime_settings = RuntimeSettings::generate(settings, size.width, size.height, &mut OsRng);

    let settings_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Runtime settings buffer"),
        contents: bytemuck::bytes_of(&runtime_settings),
        usage: BufferUsages::UNIFORM | BufferUsages::MAP_WRITE,
    });

    // Create a bind group for each orientation of the particle buffers,
    // and then make an iterator which swaps between them.
    let bind_groups = [
        device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &settings_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &particle_buffers[0],
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &particle_buffers[1],
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        }),
        device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &settings_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &particle_buffers[1],
                        offset: 0,
                        size: None,
                    }),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &particle_buffers[0],
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        }),
    ];

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
    };

    surface.configure(&device, &surface_config);

    let mut step_number = 0;

    // Spawn a task which will regularly queue compute passes.
    {

    }

    event_loop.run(move |event, _, control_flow| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&instance, &adapter, &shader, &pipeline_layout);

        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                // Reconfigure the surface with the new size
                surface_config.width = size.width;
                surface_config.height = size.height;
                surface.configure(&device, &surface_config);

                // Update the resolution given to the GPU.
                // The width/height starts at offset 8.
                queue.write_buffer(&settings_buffer, 8, bytemuck::bytes_of(&[size.width as f32, size.height as f32]));

                // Update the circle points.
                queue.write_buffer(&circle_points_buffer, 0, bytemuck::bytes_of(&circle_points(size.width, size.height)));
            }
            Event::MainEventsCleared => {
                device.poll(Maintain::Poll);

                let frame = surface
                    .get_current_frame()
                    .expect("Failed to acquire next swap chain texture")
                    .output;

                let multisampled = device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: None,
                        dimension: wgpu::TextureDimension::D2,
                        format: swapchain_format,
                        mip_level_count: 1,
                        sample_count: MULTISAMPLE_COUNT,
                        size: wgpu::Extent3d {
                            width: surface_config.width,
                            height: surface_config.height,
                            ..Default::default()
                        },
                        usage: TextureUsages::RENDER_ATTACHMENT,
                    })
                    .create_view(&wgpu::TextureViewDescriptor::default());

                let view = frame
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                let bind_group = &bind_groups[step_number % 2];

                {
                    let mut cpass =
                        encoder.begin_compute_pass(&ComputePassDescriptor { label: None });
                    cpass.set_pipeline(&compute_pipeline);
                    cpass.set_bind_group(0, &bind_group, &[]);
                    cpass.dispatch(settings.particles as u32 / 100, 1, 1);
                }

                {
                    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: None,
                        color_attachments: &[wgpu::RenderPassColorAttachment {
                            view: &multisampled,
                            resolve_target: Some(&view),
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: true,
                            },
                        }],
                        depth_stencil_attachment: None,
                    });
                    rpass.set_pipeline(&render_pipeline);
                    rpass.set_bind_group(0, &bind_group, &[]);
                    rpass.set_vertex_buffer(0, particle_buffers[step_number % 2].slice(..));
                    rpass.set_vertex_buffer(1, circle_points_buffer.slice(..));
                    rpass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    rpass.draw_indexed(
                        0..CIRCLE_POINTS as u32 * 3,
                        0,
                        0..settings.particles as u32,
                    );
                }

                queue.submit(Some(encoder.finish()));

                //TODO: vsync and all that
                // std::thread::sleep(std::time::Duration::from_millis(16));


                step_number += 1;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

fn main() {
    let event_loop = EventLoop::new();
    let window = Window::new(&event_loop).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();

        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;

        console_error_panic_hook::init_once();
        console_log::init().expect("could not initialize logger");

        // On wasm, append the canvas to the document body
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");

        wasm_bindgen_futures::spawn_local(run(event_loop, window));
    }
}
