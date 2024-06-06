use crate::{
	camera, light,
	model::{self, DrawModel, Instance},
	texture::{self, Material},
};

use std::{sync::Arc, time};

use glam::Vec3;
use winit::{
	application::ApplicationHandler,
	event::{DeviceEvent, KeyEvent, MouseButton, WindowEvent},
	event_loop::{ActiveEventLoop, EventLoop},
	keyboard::{KeyCode, PhysicalKey},
	window::{CursorGrabMode, Fullscreen, Window, WindowAttributes, WindowId},
};

pub struct App {
	state: Option<State>,
	on_ready: fn(&mut State),
}

impl App {
	pub fn new(on_ready: fn(&mut State)) -> Self {
		Self {
			state: None,
			on_ready,
		}
	}

	/// Runs the application.
	///
	/// This function will block the current thread until the application is closed.
	///
	/// # Errors
	/// See [`winit::error::EventLoopError`] for errors.
	pub fn run(mut self) -> Result<(), winit::error::EventLoopError> {
		let event_loop = EventLoop::new()?;

		event_loop.run_app(&mut self)
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
			buffers: &[model::Vertex::desc(), Instance::desc()],
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

pub struct State {
	pub window: Arc<Window>,
	pub device: wgpu::Device,
	pub queue: wgpu::Queue,
	pub surface: wgpu::Surface<'static>,
	pub config: wgpu::SurfaceConfiguration,

	pub material_bind_group_layout: wgpu::BindGroupLayout,

	opaque_render_pipeline: wgpu::RenderPipeline,
	transparent_render_pipeline: wgpu::RenderPipeline,

	pub controller: camera::CameraController,

	depth_texture: texture::Texture,
	pub models: Vec<model::GpuModel>,

	light: light::GpuLight,

	last_frame: time::Instant,
}

impl State {
	async fn new(window: Window) -> Self {
		let window = Arc::new(window);
		let size = window.inner_size();
		let instance = wgpu::Instance::default();

		let surface = instance.create_surface(window.clone()).unwrap();
		let adapter = instance
			.request_adapter(&wgpu::RequestAdapterOptions {
				power_preference: wgpu::PowerPreference::default(),
				force_fallback_adapter: false,
				compatible_surface: Some(&surface),
			})
			.await
			.expect("failed to find an appropriate adapter");

		let (device, queue) = adapter
			.request_device(
				&wgpu::DeviceDescriptor {
					label: None,
					required_features: wgpu::Features::empty(),
					required_limits: wgpu::Limits::downlevel_defaults()
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

		let shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/shader.wgsl"));

		let material_bind_group_layout = Material::create_bind_group_layout(&device);

		let camera_builder = camera::CameraBuilder::new(config.width as f32, config.height as f32);
		let camera = camera::Camera::new(&camera_builder).create_on_device(&device);
		let controller = camera::CameraController::new(camera, camera_builder);

		let light = light::Light::from_position(Vec3::new(0.0, 1.0, 0.0)).create_on_device(&device);

		let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
			label: None,
			bind_group_layouts: &[
				&material_bind_group_layout,
				&controller.camera.bind_group_layout,
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

		let transparent_render_pipeline =
			create_render_pipeline(&device, &pipeline_layout, transparent_target, true, &shader);

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

			material_bind_group_layout,

			controller,
			depth_texture,

			models: Vec::new(),
			light,

			last_frame: time::Instant::now(),
		}
	}

	fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
		self.config.width = size.width.max(1);
		self.config.height = size.height.max(1);

		self.controller
			.builder
			.projection
			.resize(self.config.width as f32, self.config.height as f32);
		self.depth_texture
			.resize(&self.device, self.config.width, self.config.height);

		self.surface.configure(&self.device, &self.config);
		self.window.request_redraw();
	}

	fn update(&mut self, delta: time::Duration) {
		self.controller.update(delta);
		self.controller
			.camera
			.update_view_proj(&self.controller.builder, &self.queue);
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

		rpass.set_pipeline(&self.opaque_render_pipeline);

		for model in &self.models {
			rpass.draw_model_instanced(
				model,
				&self.controller.camera.bind_group,
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

		rpass.set_pipeline(&self.transparent_render_pipeline);

		for model in &self.models {
			rpass.draw_model_instanced(
				model,
				&self.controller.camera.bind_group,
				&self.light.bind_group,
				true,
			);
		}

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

		window.set_cursor_grab(CursorGrabMode::Locked).unwrap();
		window.set_cursor_visible(false);

		let mut state = pollster::block_on(State::new(window));

		(self.on_ready)(&mut state);
		self.state = Some(state);
	}

	fn device_event(
		&mut self,
		_event_loop: &ActiveEventLoop,
		_device_id: winit::event::DeviceId,
		event: DeviceEvent,
	) {
		let Some(app) = self.state.as_mut() else {
			return;
		};

		if let DeviceEvent::MouseMotion { delta } = event {
			app.controller
				.process_mouse((delta.0 as f32, delta.1 as f32));
		}
	}

	fn window_event(
		&mut self,
		event_loop: &ActiveEventLoop,
		window_id: WindowId,
		event: WindowEvent,
	) {
		let Some(app) = self.state.as_mut() else {
			return;
		};

		if window_id != app.window.id() {
			return;
		}

		match event {
			WindowEvent::Resized(size) => {
				app.resize(size);
			}
			WindowEvent::RedrawRequested => {
				app.window.request_redraw();

				let delta = app.last_frame.elapsed();

				app.last_frame = time::Instant::now();
				app.update(delta);

				match app.render() {
					Ok(..) => {}
					Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
						app.resize(app.window.inner_size());
					}
					Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
					Err(wgpu::SurfaceError::Timeout) => log::warn!("surface timeout"),
				}

				std::thread::sleep(time::Duration::from_millis(10));
			}
			WindowEvent::CloseRequested => {
				event_loop.exit();
			}
			WindowEvent::KeyboardInput {
				event:
					KeyEvent {
						state,
						physical_key: PhysicalKey::Code(key),
						..
					},
				..
			} => {
				if app.controller.process_keyboard(key, state) {
					return;
				}

				if state.is_pressed() {
					match key {
						KeyCode::Escape => {
							app.window.set_cursor_grab(CursorGrabMode::None).unwrap();
							app.window.set_cursor_visible(true);
						}
						KeyCode::F11 if app.window.fullscreen().is_none() => {
							app.window
								.set_fullscreen(Some(Fullscreen::Borderless(None)));
						}
						KeyCode::F11 => {
							app.window.set_fullscreen(None);
						}
						_ => {}
					}
				}
			}
			WindowEvent::MouseWheel { delta, .. } => {
				app.controller.process_scroll(&delta);
			}
			WindowEvent::MouseInput {
				state,
				button: MouseButton::Left,
				..
			} => {
				app.controller.mouse_pressed = state.is_pressed();
			}
			_ => {}
		}
	}
}
