use std::f32::consts::TAU;
use std::mem;
use std::mem::size_of;
use std::num::NonZeroU64;

use bytemuck::Pod;
use bytemuck::Zeroable;
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
use wgpu::BufferDescriptor;
use wgpu::BufferUsages;
use wgpu::ComputePassDescriptor;
use wgpu::ComputePipelineDescriptor;
use wgpu::Limits;
use wgpu::PipelineLayoutDescriptor;
use wgpu::ShaderStages;
use wgpu::TextureUsages;
use winit::event::Event;
use winit::event::WindowEvent;
use winit::event_loop::ControlFlow;
use winit::event_loop::EventLoop;
use winit::window::Window;

use crate::kinds::ParticleKinds;
use crate::kinds::SymmetricProperties;
use crate::settings::Settings;

mod kinds;
mod settings;

const RADIUS: f32 = 10.0;
const DIAMETER: f32 = RADIUS * 2.0;
const CIRCLE_POINTS: usize = 24;
const MULTISAMPLE_COUNT: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Particle {
    pos: [f32; 2],
    kind: u32,
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

fn gen_random_particles<R: Rng>(
    width: u32,
    height: u32,
    settings: Settings,
    rng: &mut R,
) -> Vec<Particle> {
    let kinds = Uniform::new(0, settings.kinds as u32);
    let x_dist = Uniform::new(width as f32 * 0.25, width as f32 * 0.75);
    let y_dist = Uniform::new(height as f32 * 0.25, height as f32 * 0.75);

    let mut particles = Vec::with_capacity(settings.particles);
    for _ in 0..settings.particles {
        particles.push(Particle {
            kind: kinds.sample(rng),
            pos: [x_dist.sample(rng), y_dist.sample(rng)],
        })
    }
    particles
}

async fn run(event_loop: EventLoop<()>, window: Window) {
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
            // colors
            BindGroupLayoutEntry {
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<[f32; 3]>() as u64),
                },
                binding: 0,
                // NOTE: the compute shader only uses the length of the array to check how many particles there are; should that just be provided separately?
                visibility: ShaderStages::VERTEX | ShaderStages::COMPUTE,
                // TODO: is this supposed to be Some? I'm not sure if it only applies to fixed-length arrays.
                count: None,
            },
            // symmetric_properties
            BindGroupLayoutEntry {
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<SymmetricProperties>() as u64),
                },
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                // TODO: is this supposed to be Some? I'm not sure if it only applies to fixed-length arrays.
                count: None,
            },
            // attractions
            BindGroupLayoutEntry {
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<[f32; 2]>() as u64),
                },
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                // TODO: is this supposed to be Some? I'm not sure if it only applies to fixed-length arrays.
                count: None,
            },
            // velocities
            BindGroupLayoutEntry {
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<[f32; 2]>() as u64),
                },
                binding: 3,
                visibility: ShaderStages::COMPUTE,
                // TODO: is this supposed to be Some? I'm not sure if it only applies to fixed-length arrays.
                count: None,
            },
            // back_velocities
            BindGroupLayoutEntry {
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<[f32; 2]>() as u64),
                },
                binding: 4,
                visibility: ShaderStages::COMPUTE,
                // TODO: is this supposed to be Some? I'm not sure if it only applies to fixed-length arrays.
                count: None,
            },
            // particles
            BindGroupLayoutEntry {
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<Particle>() as u64),
                },
                binding: 5,
                visibility: ShaderStages::COMPUTE,
                // TODO: is this supposed to be Some? I'm not sure if it only applies to fixed-length arrays.
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
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
                },
                // The circle points
                wgpu::VertexBufferLayout {
                    array_stride: size_of::<[f32; 2]>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![2 => Float32x2],
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

    let circle_points_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Circle points buffer"),
        contents: bytemuck::bytes_of(&circle_points(size.width, size.height)),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
    });

    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Circle points index buffer"),
        contents: bytemuck::bytes_of(&indices()),
        usage: BufferUsages::INDEX,
    });

    let particles = gen_random_particles(size.width, size.height, Settings::balanced(), &mut OsRng);

    let particle_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Particle buffer"),
        contents: bytemuck::cast_slice(&particles),
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::STORAGE,
    });

    let velocities = gen_random_velocities(Settings::balanced(), &mut OsRng);

    let mut velocity_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Velocity buffer 1"),
        contents: bytemuck::cast_slice(&velocities),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });

    let mut back_velocity_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("Velocity buffer 2"),
        mapped_at_creation: false,
        size: (Settings::balanced().particles * size_of::<[f32; 2]>()) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });

    let particle_kinds = ParticleKinds::random(Settings::balanced(), &mut OsRng);

    let color_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Particle colors buffer"),
        contents: bytemuck::cast_slice(&particle_kinds.colors),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });

    let attraction_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Attractions buffer"),
        contents: bytemuck::cast_slice(&particle_kinds.attractions),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });

    let symmetric_buffer = device.create_buffer_init(&BufferInitDescriptor {
        label: Some("Symmetric properties buffer"),
        contents: bytemuck::cast_slice(&particle_kinds.symmetric_properties),
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
    });

    let bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &color_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &symmetric_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &attraction_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &velocity_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            BindGroupEntry {
                binding: 4,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &back_velocity_buffer,
                    offset: 0,
                    size: None,
                }),
            },
            BindGroupEntry {
                binding: 5,
                resource: BindingResource::Buffer(BufferBinding {
                    buffer: &particle_buffer,
                    offset: 0,
                    size: None,
                }),
            },
        ],
    });

    let mut surface_config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: swapchain_format,
        width: size.width,
        height: size.height,
        present_mode: wgpu::PresentMode::Fifo,
    };

    surface.configure(&device, &surface_config);

    event_loop.run(move |event, _, control_flow| {
        // Have the closure take ownership of the resources.
        // `event_loop.run` never returns, therefore we must do this to ensure
        // the resources are properly cleaned up.
        let _ = (&instance, &adapter, &shader, &pipeline_layout);

        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                // Reconfigure the surface with the new size
                surface_config.width = size.width;
                surface_config.height = size.height;
                surface.configure(&device, &surface_config);
            }
            Event::RedrawRequested(_) => {
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

                {
                    let mut cpass =
                        encoder.begin_compute_pass(&ComputePassDescriptor { label: None });
                    cpass.set_pipeline(&compute_pipeline);
                    cpass.set_bind_group(0, &bind_group, &[]);
                    cpass.dispatch(Settings::balanced().particles as u32 / 100, 1, 1);
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
                    rpass.set_vertex_buffer(0, particle_buffer.slice(..));
                    rpass.set_vertex_buffer(1, circle_points_buffer.slice(..));
                    rpass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                    rpass.draw_indexed(0..CIRCLE_POINTS as u32 * 3, 0, 0..2);
                }

                queue.submit(Some(encoder.finish()));

                // swap the velocities around for next time
                mem::swap(&mut velocity_buffer, &mut back_velocity_buffer);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}

fn gen_random_velocities(settings: Settings, rng: &mut OsRng) -> Vec<[f32; 2]> {
    let dist = Normal::new(0.0, 0.2).unwrap();

    let mut velocities = Vec::with_capacity(settings.particles);
    for _ in 0..settings.particles {
        velocities.push([dist.sample(rng), dist.sample(rng)]);
    }
    velocities
}

fn main() {
    let event_loop = EventLoop::new();
    let window = winit::window::Window::new(&event_loop).unwrap();
    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        // Temporarily avoid srgb formats for the swapchain on the web
        pollster::block_on(run(event_loop, window));
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init().expect("could not initialize logger");
        use winit::platform::web::WindowExtWebSys;
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
