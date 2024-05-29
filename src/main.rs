mod camera;
mod texture;

use std::{mem, sync::Arc};

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;
use winit::{
	application::ApplicationHandler,
	event::WindowEvent,
	event_loop::{ActiveEventLoop, EventLoop},
	window::{Window, WindowAttributes, WindowId},
};

#[derive(Default)]
struct App {
	state: Option<State>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Zeroable, Pod)]
struct Vertex {
	position: [f32; 3],
	tex_coords: [f32; 2],
}

impl Vertex {
	const ATTRIBUTES: [wgpu::VertexAttribute; 2] =
		wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2];

	fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &Self::ATTRIBUTES,
		}
	}
}

mod triangle {
	use super::Vertex;

	pub const VERTICES: &[Vertex] = &[
		// Changed
		Vertex {
			position: [-0.0868241, 0.49240386, 0.0],
			tex_coords: [0.4131759, 0.00759614],
		}, // A
		Vertex {
			position: [-0.49513406, 0.06958647, 0.0],
			tex_coords: [0.0048659444, 0.43041354],
		}, // B
		Vertex {
			position: [-0.21918549, -0.44939706, 0.0],
			tex_coords: [0.28081453, 0.949397],
		}, // C
		Vertex {
			position: [0.35966998, -0.3473291, 0.0],
			tex_coords: [0.85967, 0.84732914],
		}, // D
		Vertex {
			position: [0.44147372, 0.2347359, 0.0],
			tex_coords: [0.9414737, 0.2652641],
		}, // E
	];

	pub const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];
}

mod cube {
	use super::Vertex;

	pub const VERTICES: &[Vertex] = &[
		Vertex {
			position: [-1.0, -1.0, 1.0],
			tex_coords: [0.0, 0.0],
		},
		Vertex {
			position: [1.0, -1.0, 1.0],
			tex_coords: [1.0, 0.0],
		},
		Vertex {
			position: [1.0, 1.0, 1.0],
			tex_coords: [1.0, 1.0],
		},
		Vertex {
			position: [-1.0, 1.0, 1.0],
			tex_coords: [0.0, 1.0],
		},
		// Back face
		Vertex {
			position: [-1.0, -1.0, -1.0],
			tex_coords: [1.0, 0.0],
		},
		Vertex {
			position: [1.0, -1.0, -1.0],
			tex_coords: [0.0, 0.0],
		},
		Vertex {
			position: [1.0, 1.0, -1.0],
			tex_coords: [0.0, 1.0],
		},
		Vertex {
			position: [-1.0, 1.0, -1.0],
			tex_coords: [1.0, 1.0],
		},
		// Left face
		Vertex {
			position: [-1.0, -1.0, -1.0],
			tex_coords: [0.0, 0.0],
		},
		Vertex {
			position: [-1.0, -1.0, 1.0],
			tex_coords: [1.0, 0.0],
		},
		Vertex {
			position: [-1.0, 1.0, 1.0],
			tex_coords: [1.0, 1.0],
		},
		Vertex {
			position: [-1.0, 1.0, -1.0],
			tex_coords: [0.0, 1.0],
		},
		// Right face
		Vertex {
			position: [1.0, -1.0, -1.0],
			tex_coords: [1.0, 0.0],
		},
		Vertex {
			position: [1.0, -1.0, 1.0],
			tex_coords: [0.0, 0.0],
		},
		Vertex {
			position: [1.0, 1.0, 1.0],
			tex_coords: [0.0, 1.0],
		},
		Vertex {
			position: [1.0, 1.0, -1.0],
			tex_coords: [1.0, 1.0],
		},
		// Top face
		Vertex {
			position: [-1.0, 1.0, -1.0],
			tex_coords: [0.0, 1.0],
		},
		Vertex {
			position: [-1.0, 1.0, 1.0],
			tex_coords: [0.0, 0.0],
		},
		Vertex {
			position: [1.0, 1.0, 1.0],
			tex_coords: [1.0, 0.0],
		},
		Vertex {
			position: [1.0, 1.0, -1.0],
			tex_coords: [1.0, 1.0],
		},
		// Bottom face
		Vertex {
			position: [-1.0, -1.0, -1.0],
			tex_coords: [1.0, 1.0],
		},
		Vertex {
			position: [-1.0, -1.0, 1.0],
			tex_coords: [1.0, 0.0],
		},
		Vertex {
			position: [1.0, -1.0, 1.0],
			tex_coords: [0.0, 0.0],
		},
		Vertex {
			position: [1.0, -1.0, -1.0],
			tex_coords: [0.0, 1.0],
		},
	];

	pub const INDICES: &[u16] = &[
		// Front face
		0, 1, 2, 0, 2, 3, // Back face
		4, 5, 6, 4, 6, 7, // Left face
		8, 9, 10, 8, 10, 11, // Right face
		12, 13, 14, 12, 14, 15, // Top face
		16, 17, 18, 16, 18, 19, // Bottom face
		20, 21, 22, 20, 22, 23,
	];
}

struct State {
	window: Arc<Window>,
	device: wgpu::Device,
	queue: wgpu::Queue,
	surface: wgpu::Surface<'static>,
	render_pipeline: wgpu::RenderPipeline,
	config: wgpu::SurfaceConfiguration,

	vertex_buffer: wgpu::Buffer,
	index_buffer: wgpu::Buffer,
	num_indices: u32,
	diffuse_bind_group: wgpu::BindGroup,
	diffuse_texture: texture::Texture,

	camera: camera::Camera,
	camera_buffer: wgpu::Buffer,
	camera_bind_group: wgpu::BindGroup,
	camera_uniform: camera::CameraUniform,
}

impl State {
	async fn new(window: Window) -> Self {
		let window = Arc::new(window);

		let mut size = window.inner_size();
		size.width = size.width.max(1);
		size.height = size.height.max(1);

		let instance = wgpu::Instance::default();

		let surface = instance.create_surface(window.clone()).unwrap();
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(),
				force_fallback_adapter: false,
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
					required_features: wgpu::Features::empty(),
					// Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
					required_limits: wgpu::Limits::downlevel_webgl2_defaults()
						.using_resolution(adapter.limits()),
				},
				None,
			)
			.await
			.expect("Failed to create device");

		let config = surface
			.get_default_config(&adapter, size.width, size.height)
			.unwrap();
		surface.configure(&device, &config);

		let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Vertex Buffer"),
			contents: bytemuck::cast_slice(triangle::VERTICES),
			usage: wgpu::BufferUsages::VERTEX,
		});

		let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Index Buffer"),
			contents: bytemuck::cast_slice(triangle::INDICES),
			usage: wgpu::BufferUsages::INDEX,
		});

		// Load the shaders from disk
		let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/triangle.wgsl"));

		let texture_bind_group_layout =
			device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				entries: &[
					wgpu::BindGroupLayoutEntry {
						binding: 0,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Texture {
							multisampled: false,
							view_dimension: wgpu::TextureViewDimension::D2,
							sample_type: wgpu::TextureSampleType::Float { filterable: true },
						},
						count: None,
					},
					wgpu::BindGroupLayoutEntry {
						binding: 1,
						visibility: wgpu::ShaderStages::FRAGMENT,
						// This should match the filterable field of the
						// corresponding Texture entry above.
						ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
						count: None,
					},
				],
				label: Some("texture_bind_group_layout"),
			});

		let camera = camera::Camera::new(config.width as f32, config.height as f32);
		let camera_uniform = camera::CameraUniform::new(&camera);

		let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Camera Buffer"),
			contents: bytemuck::cast_slice(&[camera_uniform]),
			usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
		});

		let camera_bind_group_layout =
			device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				entries: &[wgpu::BindGroupLayoutEntry {
					binding: 0,
					visibility: wgpu::ShaderStages::VERTEX,
					ty: wgpu::BindingType::Buffer {
						ty: wgpu::BufferBindingType::Uniform,
						has_dynamic_offset: false,
						min_binding_size: None,
					},
					count: None,
				}],
				label: Some("camera_bind_group_layout"),
			});

		let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &camera_bind_group_layout,
			entries: &[wgpu::BindGroupEntry {
				binding: 0,
				resource: camera_buffer.as_entire_binding(),
			}],
			label: Some("camera_bind_group"),
		});

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &[&texture_bind_group_layout, &camera_bind_group_layout],
			push_constant_ranges: &[],
		});

		let swapchain_capabilities = surface.get_capabilities(&adapter);
		let swapchain_format = swapchain_capabilities.formats[0];

		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: None,
			layout: Some(&pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: "vs_main",
				buffers: &[Vertex::desc()],
				compilation_options: Default::default(),
			},
			fragment: Some(wgpu::FragmentState {
				module: &shader,
				entry_point: "fs_main",
				compilation_options: Default::default(),
				targets: &[Some(swapchain_format.into())],
			}),
			primitive: wgpu::PrimitiveState {
				conservative: false,
				topology: wgpu::PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: Some(wgpu::Face::Back),
				polygon_mode: wgpu::PolygonMode::Fill,
				unclipped_depth: false,
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState::default(),
			multiview: None,
		});

		let diffuse_bytes = include_bytes!("../tree.png");
		let diffuse_texture =
			texture::Texture::from_bytes(&device, &queue, diffuse_bytes, "happy-tree.png").unwrap();

		let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
			layout: &texture_bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry {
					binding: 0,
					resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
				},
				wgpu::BindGroupEntry {
					binding: 1,
					resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
				},
			],
			label: Some("diffuse_bind_group"),
		});

		Self {
			window,
			device,
			queue,
			surface,
			render_pipeline,
			config,
			vertex_buffer,
			index_buffer,
			num_indices: triangle::INDICES.len() as u32,
			diffuse_bind_group,
			diffuse_texture,

			camera,
			camera_buffer,
			camera_bind_group,
			camera_uniform,
		}
	}

	fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
		self.config.width = size.width.max(1);
		self.config.height = size.height.max(1);

		self.surface.configure(&self.device, &self.config);
		self.window.request_redraw();
	}

	fn render(&self) -> Result<(), wgpu::SurfaceError> {
		let frame = self.surface.get_current_texture()?;
		let view = frame
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());
		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

		let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
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

		rpass.set_pipeline(&self.render_pipeline);
		rpass.set_bind_group(0, &self.diffuse_bind_group, &[]);
		rpass.set_bind_group(1, &self.camera_bind_group, &[]);
		rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
		rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
		rpass.draw_indexed(0..self.num_indices, 0, 0..1);

		drop(rpass);

		self.queue.submit(Some(encoder.finish()));
		frame.present();

		Ok(())
	}
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let window = event_loop
			.create_window(WindowAttributes::default().with_title("Triangle"))
			.unwrap();
		let state = pollster::block_on(State::new(window));

		self.state = Some(state);
	}

	fn window_event(
		&mut self,
		event_loop: &ActiveEventLoop,
		window_id: WindowId,
		event: WindowEvent,
	) {
		let Some(state) = self.state.as_mut() else {
			return;
		};

		if window_id != state.window.id() {
			return;
		}

		match event {
			WindowEvent::Resized(size) => {
				state.resize(size);
			}
			WindowEvent::RedrawRequested => {
				state.render().unwrap();
			}
			WindowEvent::CloseRequested => {
				event_loop.exit();
			}
			_ => {}
		}
	}
}

fn main() -> Result<(), winit::error::EventLoopError> {
	env_logger::init();

	let event_loop = EventLoop::new().unwrap();
	let mut app = App::default();

	event_loop.run_app(&mut app)
}
