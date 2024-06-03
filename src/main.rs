pub use ira::*;

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use model::DrawModel;
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

struct Instance {
	position: Vec3,
	rotation: Quat,
}

impl Instance {
	fn to_raw(&self) -> InstanceRaw {
		InstanceRaw {
			model: (Mat4::from_translation(self.position) * Mat4::from_quat(self.rotation))
				.to_cols_array_2d(),
		}
	}
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct InstanceRaw {
	model: [[f32; 4]; 4],
}

impl InstanceRaw {
	const VERTICES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
		5 => Float32x4,
		6 => Float32x4,
		7 => Float32x4,
		8 => Float32x4,
	];

	fn desc() -> wgpu::VertexBufferLayout<'static> {
		use std::mem;
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &Self::VERTICES,
		}
	}
}

fn create_render_pipeline(
	device: &wgpu::Device,
	layout: &wgpu::PipelineLayout,
	target: wgpu::ColorTargetState,
	depth_write_enabled: bool,
	shader: &wgpu::ShaderModule,
) -> wgpu::RenderPipeline {
	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: None,
		layout: Some(layout),
		vertex: wgpu::VertexState {
			module: shader,
			entry_point: "vs_main",
			buffers: &[model::Vertex::desc(), InstanceRaw::desc()],
			compilation_options: Default::default(),
		},
		fragment: Some(wgpu::FragmentState {
			module: shader,
			entry_point: "fs_main",
			compilation_options: Default::default(),
			targets: &[Some(target)],
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
		depth_stencil: Some(wgpu::DepthStencilState {
			format: texture::Texture::DEPTH_FORMAT,
			depth_write_enabled,
			depth_compare: wgpu::CompareFunction::Less,
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}),
		multisample: wgpu::MultisampleState {
			count: 1,
			mask: !0,
			alpha_to_coverage_enabled: false,
		},
		multiview: None,
	})
}

struct State {
	window: Arc<Window>,
	device: wgpu::Device,
	queue: wgpu::Queue,
	surface: wgpu::Surface<'static>,
	config: wgpu::SurfaceConfiguration,

	opaque_render_pipeline: wgpu::RenderPipeline,
	transparent_render_pipeline: wgpu::RenderPipeline,

	camera: camera::Camera,
	camera_buffer: wgpu::Buffer,
	camera_bind_group: wgpu::BindGroup,
	camera_uniform: camera::CameraUniform,
	camera_controller: camera::CameraController,

	depth_texture: texture::Texture,

	instances: Vec<Instance>,
	instance_buffer: wgpu::Buffer,
	models: Vec<model::Model>,

	light: light::GpuLight,
}

const NUM_INSTANCES_PER_ROW: u32 = 10;

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

		let depth_texture =
			texture::Texture::create_depth_texture(&device, &config, "depth_texture");

		// Load the shaders from disk
		let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/shader.wgsl"));

		const SPACE_BETWEEN: f32 = 3.0;
		let instances = (0..NUM_INSTANCES_PER_ROW)
			.flat_map(|z| {
				(0..NUM_INSTANCES_PER_ROW).map(move |x| {
					let x = SPACE_BETWEEN * (x as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);
					let z = SPACE_BETWEEN * (z as f32 - NUM_INSTANCES_PER_ROW as f32 / 2.0);

					let position = Vec3 { x, y: 0.0, z };

					let rotation = if position == Vec3::ZERO {
						Quat::from_axis_angle(Vec3::Z, 0.0)
					} else {
						Quat::from_axis_angle(position.normalize(), 45f32.to_radians())
					};

					Instance { position, rotation }
				})
			})
			.collect::<Vec<_>>();

		let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
		let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Instance Buffer"),
			contents: bytemuck::cast_slice(&instance_data),
			usage: wgpu::BufferUsages::VERTEX,
		});

		let texture_bind_group_layout =
			device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
				entries: &[
					// Diffuse
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
					// Normal
					wgpu::BindGroupLayoutEntry {
						binding: 2,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Texture {
							sample_type: wgpu::TextureSampleType::Float { filterable: true },
							view_dimension: wgpu::TextureViewDimension::D2,
							multisampled: false,
						},
						count: None,
					},
					wgpu::BindGroupLayoutEntry {
						binding: 3,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
						count: None,
					},
					// Metallic + Roughness
					wgpu::BindGroupLayoutEntry {
						binding: 4,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Texture {
							sample_type: wgpu::TextureSampleType::Float { filterable: true },
							view_dimension: wgpu::TextureViewDimension::D2,
							multisampled: false,
						},
						count: None,
					},
					wgpu::BindGroupLayoutEntry {
						binding: 5,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
						count: None,
					},
					// AO
					wgpu::BindGroupLayoutEntry {
						binding: 6,
						visibility: wgpu::ShaderStages::FRAGMENT,
						ty: wgpu::BindingType::Texture {
							sample_type: wgpu::TextureSampleType::Float { filterable: true },
							view_dimension: wgpu::TextureViewDimension::D2,
							multisampled: false,
						},
						count: None,
					},
					wgpu::BindGroupLayoutEntry {
						binding: 7,
						visibility: wgpu::ShaderStages::FRAGMENT,
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
					visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
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

		let model = model::Model::from_path(
			"models/bottled_car/scene.gltf",
			&device,
			&queue,
			&texture_bind_group_layout,
		)
		.await
		.unwrap();

		let light = light::Light::from_model(&model).create_on_device(&device);

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &[
				&texture_bind_group_layout,
				&camera_bind_group_layout,
				&light.bind_group_layout,
			],
			push_constant_ranges: &[],
		});

		let swapchain_capabilities = surface.get_capabilities(&adapter);
		let transparent_target = wgpu::ColorTargetState {
			format: swapchain_capabilities.formats[0],
			blend: Some(wgpu::BlendState {
				color: wgpu::BlendComponent {
					src_factor: wgpu::BlendFactor::SrcAlpha,
					dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
					operation: wgpu::BlendOperation::Add,
				},
				alpha: wgpu::BlendComponent {
					src_factor: wgpu::BlendFactor::One,
					dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
					operation: wgpu::BlendOperation::Add,
				},
			}),
			write_mask: wgpu::ColorWrites::COLOR,
		};

		let transparent_render_pipeline = create_render_pipeline(
			&device,
			&pipeline_layout,
			transparent_target,
			false,
			&shader,
		);

		let opaque_render_pipeline = create_render_pipeline(
			&device,
			&pipeline_layout,
			wgpu::ColorTargetState {
				format: swapchain_capabilities.formats[0],
				blend: None,
				write_mask: wgpu::ColorWrites::ALL,
			},
			true,
			&shader,
		);

		Self {
			window,
			device,
			queue,
			surface,
			config,

			opaque_render_pipeline,
			transparent_render_pipeline,

			camera,
			camera_buffer,
			camera_bind_group,
			camera_uniform,
			camera_controller: camera::CameraController::new(0.2),

			depth_texture,

			instances,
			instance_buffer,
			models: vec![model],
			light,
		}
	}

	fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
		self.config.width = size.width.max(1);
		self.config.height = size.height.max(1);

		self.surface.configure(&self.device, &self.config);
		self.window.request_redraw();
	}

	fn update(&mut self) {
		self.light
			.set_position(self.camera_uniform.view_pos, &self.queue);
	}

	fn render(&self) -> Result<(), wgpu::SurfaceError> {
		let start = std::time::Instant::now();

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
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: &self.depth_texture.view,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Clear(1.0),
					store: wgpu::StoreOp::Store,
				}),
				stencil_ops: None,
			}),
			timestamp_writes: None,
			occlusion_query_set: None,
		});

		rpass.set_vertex_buffer(1, self.instance_buffer.slice(..));
		rpass.set_pipeline(&self.opaque_render_pipeline);

		for model in &self.models {
			rpass.draw_model_instanced(
				model,
				0..self.instances.len() as u32,
				&self.camera_bind_group,
				&self.light.bind_group,
				false,
			);
		}

		drop(rpass);

		let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view: &view,
				resolve_target: None,
				ops: wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: wgpu::StoreOp::Store,
				},
			})],
			depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
				view: &self.depth_texture.view,
				depth_ops: Some(wgpu::Operations {
					load: wgpu::LoadOp::Load,
					store: wgpu::StoreOp::Store,
				}),
				stencil_ops: None,
			}),
			timestamp_writes: None,
			occlusion_query_set: None,
		});

		rpass.set_vertex_buffer(1, self.instance_buffer.slice(..));
		rpass.set_pipeline(&self.transparent_render_pipeline);

		for model in &self.models {
			rpass.draw_model_instanced(
				model,
				0..self.instances.len() as u32,
				&self.camera_bind_group,
				&self.light.bind_group,
				true,
			);
		}

		drop(rpass);

		self.queue.submit(Some(encoder.finish()));
		frame.present();

		log::info!("Frame time: {:?}", start.elapsed());

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

		if state.camera_controller.process_events(&event) {
			state.camera_controller.update_camera(&mut state.camera);
			state.camera_uniform.update_view_proj(&state.camera);
			state.queue.write_buffer(
				&state.camera_buffer,
				0,
				bytemuck::cast_slice(&[state.camera_uniform]),
			);

			state.window.request_redraw();

			return;
		}

		match event {
			WindowEvent::Resized(size) => {
				state.resize(size);
			}
			WindowEvent::RedrawRequested => {
				state.update();
				state.render().unwrap();

				// sleep for 10ms
				std::thread::sleep(std::time::Duration::from_millis(10));
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
