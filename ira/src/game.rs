use crate::{
	camera, light, model::Instance, physics::PhysicsState, Camera, DrumExt, GpuDrum, GpuTexture,
	MaterialExt, VertexExt,
};

use std::{
	sync::Arc,
	time::{self, Duration},
};

use glam::{Vec2, Vec3};
use ira_drum::{Drum, Material, Vertex};
use winit::{
	application::ApplicationHandler,
	event::{DeviceEvent, KeyEvent, WindowEvent},
	event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
	keyboard::{KeyCode, PhysicalKey},
	window::{CursorGrabMode, Fullscreen, Window, WindowAttributes, WindowId},
};

#[allow(unused_variables)]
pub trait App {
	/// Called once at the start of the program, right after the window
	/// is created but before anything else is done.
	fn on_init(window: &mut Window) -> Drum;
	/// Called once when everything has been created on the GPU.
	fn on_ready(ctx: &mut Context) -> Self;
	/// Called once per frame, right before rendering.
	fn on_update(&mut self, ctx: &mut Context, delta: Duration) {}
	/// Called every 1/60th of a second. If queued at the same time as an update,
	/// this will always be called first.
	fn on_fixed_update(&mut self, ctx: &mut Context) {}
}

pub struct Game<A> {
	state: Option<(Context, A)>,
}

impl<A> Default for Game<A> {
	fn default() -> Self {
		Self { state: None }
	}
}

impl<A: App> Game<A> {
	/// Runs the application.
	///
	/// This function will block the current thread until the application is closed.
	///
	/// # Errors
	///
	/// See [`winit::error::EventLoopError`] for errors.
	pub fn run(mut self) -> Result<(), winit::error::EventLoopError> {
		let event_loop = EventLoop::new()?;

		event_loop.set_control_flow(ControlFlow::Poll);
		event_loop.run_app(&mut self)
	}
}

fn create_render_pipeline(
	device: &wgpu::Device,
	layout: &wgpu::PipelineLayout,
	target: wgpu::ColorTargetState,
	depth_write_enabled: bool,
	shader: &wgpu::ShaderModule,
	sample_count: u32,
) -> wgpu::RenderPipeline {
	device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
		label: None,
		layout: Some(layout),
		vertex: wgpu::VertexState {
			module: shader,
			entry_point: "vs_main",
			buffers: &[Vertex::desc(), Instance::desc()],
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
			format: GpuTexture::DEPTH_FORMAT,
			depth_write_enabled,
			depth_compare: wgpu::CompareFunction::LessEqual,
			stencil: wgpu::StencilState::default(),
			bias: wgpu::DepthBiasState::default(),
		}),
		multisample: wgpu::MultisampleState {
			count: sample_count,
			..Default::default()
		},
		multiview: None,
	})
}

/// The game context, containing all game-related data.
pub struct Context {
	pub window: Arc<Window>,
	pub camera: Camera,

	pub(crate) device: wgpu::Device,
	pub(crate) queue: wgpu::Queue,
	pub(crate) surface: wgpu::Surface<'static>,
	pub(crate) config: wgpu::SurfaceConfiguration,

	pub(crate) brdf_bind_group: wgpu::BindGroup,

	opaque_render_pipeline: wgpu::RenderPipeline,
	transparent_render_pipeline: wgpu::RenderPipeline,

	pub(crate) depth_texture: GpuTexture,

	lights: light::Lights,

	last_frame: time::Instant,
	last_physics: time::Instant,

	pub(crate) sample_count: u32,
	pub(crate) multisampled_texture: GpuTexture,

	pub drum: GpuDrum,
	pub physics: PhysicsState,

	pub(crate) desired_fps: f32,

	pub(crate) pressed_keys: Vec<KeyCode>,
	pub(crate) just_pressed: Vec<KeyCode>,
	pub(crate) mouse_delta: Vec2,
}

async fn request_device(adapter: &wgpu::Adapter) -> (wgpu::Device, wgpu::Queue) {
	adapter
		.request_device(
			&wgpu::DeviceDescriptor {
				label: None,
				required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
					| wgpu::Features::TEXTURE_COMPRESSION_BC,
				required_limits: wgpu::Limits::downlevel_defaults()
					.using_resolution(adapter.limits()),
			},
			None,
		)
		.await
		.expect("failed to create device")
}

async fn request_adapter(instance: &wgpu::Instance, surface: &wgpu::Surface<'_>) -> wgpu::Adapter {
	instance
		.request_adapter(&wgpu::RequestAdapterOptions {
			power_preference: wgpu::PowerPreference::default(),
			force_fallback_adapter: false,
			compatible_surface: Some(surface),
		})
		.await
		.expect("failed to find an appropriate adapter")
}

#[allow(clippy::too_many_arguments)]
fn create_pbr_render_pipelines(
	device: &wgpu::Device,
	material_bind_group_layout: &wgpu::BindGroupLayout,
	brdf_bind_group_layout: &wgpu::BindGroupLayout,
	camera: &Camera,
	lights: &light::Lights,
	sample_count: u32,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
	let shader = device.create_shader_module(wgpu::include_wgsl!(concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/shaders/pbr.wgsl"
	)));

	let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
		label: None,
		bind_group_layouts: &[
			material_bind_group_layout,
			&camera.gpu.bind_group_layout,
			&lights.bind_group_layout,
			brdf_bind_group_layout,
		],
		push_constant_ranges: &[],
	});

	let opaque_render_pipeline = create_render_pipeline(
		device,
		&pipeline_layout,
		wgpu::ColorTargetState {
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			blend: None,
			write_mask: wgpu::ColorWrites::ALL,
		},
		true,
		&shader,
		sample_count,
	);

	let transparent_render_pipeline = {
		let transparent_target = wgpu::ColorTargetState {
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
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

		create_render_pipeline(
			device,
			&pipeline_layout,
			transparent_target,
			false,
			&shader,
			sample_count,
		)
	};

	(opaque_render_pipeline, transparent_render_pipeline)
}

impl Context {
	pub fn pressed(&self, key: KeyCode) -> bool {
		self.pressed_keys.contains(&key)
	}

	pub fn just_pressed(&self, key: KeyCode) -> bool {
		self.just_pressed.contains(&key)
	}

	pub fn mouse_delta(&self) -> Vec2 {
		self.mouse_delta
	}

	async fn new(window: Window, drum: Drum) -> Self {
		let window = Arc::new(window);
		let size = window.inner_size();
		let instance = wgpu::Instance::default();

		let surface = instance.create_surface(window.clone()).unwrap();
		let adapter = request_adapter(&instance, &surface).await;

		let (device, queue) = request_device(&adapter).await;

		let config = wgpu::SurfaceConfiguration {
			usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
			format: wgpu::TextureFormat::Rgba8UnormSrgb,
			width: size.width,
			height: size.height,
			desired_maximum_frame_latency: 2,
			present_mode: wgpu::PresentMode::Mailbox,
			alpha_mode: wgpu::CompositeAlphaMode::Auto,
			view_formats: vec![],
		};

		surface.configure(&device, &config);

		let sample_flags = adapter.get_texture_format_features(config.format).flags;

		let sample_count = {
			if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X16) {
				16
			} else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X8) {
				8
			} else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4) {
				4
			} else if sample_flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X2) {
				2
			} else {
				1
			}
		};

		let depth_texture =
			GpuTexture::create_depth_texture(&device, &config, sample_count, "depth_texture");

		let (brdf_bind_group, brdf_bind_group_layout) =
			drum.create_brdf_bind_group(&device, &queue);

		let physics = PhysicsState::default();
		let drum = drum.into_gpu(&device, &queue, &physics);

		let material_bind_group_layout = Material::create_bind_group_layout(&device);
		let camera_settings = camera::Settings::new(config.width as f32, config.height as f32);
		let camera = camera::CameraUniform::from_settings(&camera_settings).into_gpu(&device);
		let camera = Camera::new(camera, camera_settings);

		let lights = light::Lights::from_lights(
			&[
				light::GpuLight::from_position(Vec3::new(0.0, 2.0, 0.0)),
				light::GpuLight::from_position(Vec3::new(2.0, 0.0, 0.0)),
			],
			&device,
		);

		let (opaque_render_pipeline, transparent_render_pipeline) = create_pbr_render_pipelines(
			&device,
			&material_bind_group_layout,
			&brdf_bind_group_layout,
			&camera,
			&lights,
			sample_count,
		);

		let multisampled_texture = GpuTexture::create_multisampled(&device, &config, sample_count);

		Self {
			window,
			device,
			queue,
			surface,
			config,

			opaque_render_pipeline,
			transparent_render_pipeline,

			brdf_bind_group,

			camera,
			depth_texture,

			lights,

			last_frame: time::Instant::now(),
			last_physics: time::Instant::now(),
			sample_count,

			multisampled_texture,
			drum,
			physics,

			desired_fps: 120.0,

			pressed_keys: Vec::new(),
			just_pressed: Vec::new(),
			mouse_delta: Vec2::ZERO,
		}
	}

	fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
		self.config.width = size.width.max(1);
		self.config.height = size.height.max(1);

		self.camera
			.settings
			.projection
			.resize(self.config.width as f32, self.config.height as f32);
		self.depth_texture
			.resize(&self.device, self.config.width, self.config.height);
		self.multisampled_texture
			.resize(&self.device, self.config.width, self.config.height);

		self.surface.configure(&self.device, &self.config);
		self.window.request_redraw();
	}

	fn render(&self) -> Result<(), wgpu::SurfaceError> {
		let frame = self.surface.get_current_texture()?;
		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
		let view = if self.sample_count > 1 {
			&self.multisampled_texture.view
		} else {
			&frame
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default())
		};

		let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view,
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

		for model in &self.drum.models {
			model.draw_instanced(
				&mut rpass,
				&self.drum,
				&self.camera.gpu.bind_group,
				&self.lights.bind_group,
				&self.brdf_bind_group,
				false,
			);
		}

		drop(rpass);

		let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
			label: None,
			color_attachments: &[Some(wgpu::RenderPassColorAttachment {
				view,
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

		for model in &self.drum.models {
			model.draw_instanced(
				&mut rpass,
				&self.drum,
				&self.camera.gpu.bind_group,
				&self.lights.bind_group,
				&self.brdf_bind_group,
				true,
			);
		}

		drop(rpass);

		if self.sample_count > 1 {
			let frame_view = frame
				.texture
				.create_view(&wgpu::TextureViewDescriptor::default());

			// copy MSAA texture to frame view
			encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: None,
				color_attachments: &[Some(wgpu::RenderPassColorAttachment {
					view,
					resolve_target: Some(&frame_view),
					ops: wgpu::Operations {
						load: wgpu::LoadOp::Load,
						store: wgpu::StoreOp::Store,
					},
				})],
				depth_stencil_attachment: None,
				timestamp_writes: None,
				occlusion_query_set: None,
			});
		}

		self.queue.submit(Some(encoder.finish()));
		frame.present();

		Ok(())
	}
}

impl<A: App> Game<A> {
	fn render(&mut self, event_loop: &ActiveEventLoop) {
		let Some((ctx, app)) = self.state.as_mut() else {
			return;
		};

		let delta = ctx.last_physics.elapsed();

		// physics 60fps
		if delta.as_secs_f32() >= 1.0 / 60.0 {
			ctx.last_physics = time::Instant::now();
			ctx.physics_update();
			app.on_fixed_update(ctx);
		}

		let delta = ctx.last_frame.elapsed();

		if delta.as_secs_f32() < 1.0 / ctx.desired_fps {
			return;
		}

		ctx.last_frame = time::Instant::now();

		app.on_update(ctx, delta);
		ctx.camera.update_view_proj(&ctx.queue);

		ctx.just_pressed.clear();
		ctx.mouse_delta = Vec2::ZERO;

		for model in &mut ctx.drum.models {
			model.update_instance_buffer(&ctx.device, &ctx.queue);
		}

		match ctx.render() {
			Ok(..) => {}
			Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
				ctx.resize(ctx.window.inner_size());
			}
			Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
			Err(wgpu::SurfaceError::Timeout) => log::warn!("surface timeout"),
		}
	}
}

impl<A: App> ApplicationHandler for Game<A> {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let mut window = event_loop
			.create_window(WindowAttributes::default().with_title("Triangle"))
			.unwrap();

		window.set_cursor_grab(CursorGrabMode::Locked).unwrap();
		window.set_cursor_visible(false);

		let drum = A::on_init(&mut window);
		let mut ctx = pollster::block_on(Context::new(window, drum));
		let app = A::on_ready(&mut ctx);

		self.state = Some((ctx, app));
	}

	fn device_event(
		&mut self,
		_event_loop: &ActiveEventLoop,
		_device_id: winit::event::DeviceId,
		event: DeviceEvent,
	) {
		let Some((ctx, _)) = self.state.as_mut() else {
			return;
		};

		if let DeviceEvent::MouseMotion { delta } = event {
			ctx.mouse_delta += Vec2::from((delta.0 as f32, delta.1 as f32));
		}
	}

	fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
		self.render(event_loop);
	}

	fn window_event(
		&mut self,
		event_loop: &ActiveEventLoop,
		window_id: WindowId,
		event: WindowEvent,
	) {
		let Some((ctx, _)) = self.state.as_mut() else {
			return;
		};

		if window_id != ctx.window.id() {
			return;
		}

		match event {
			WindowEvent::Resized(size) => {
				ctx.resize(size);
			}
			WindowEvent::RedrawRequested => {
				self.render(event_loop);
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
				if state.is_pressed() {
					if !ctx.pressed_keys.contains(&key) {
						ctx.pressed_keys.push(key);
					}

					match key {
						KeyCode::Escape => {
							ctx.window.set_cursor_grab(CursorGrabMode::None).unwrap();
							ctx.window.set_cursor_visible(true);
						}
						KeyCode::F11 if ctx.window.fullscreen().is_none() => {
							ctx.window
								.set_fullscreen(Some(Fullscreen::Borderless(None)));
						}
						KeyCode::F11 => {
							ctx.window.set_fullscreen(None);
						}
						_ => {}
					}
				} else if let Some(index) = ctx.pressed_keys.iter().position(|&k| k == key) {
					ctx.pressed_keys.swap_remove(index);
				}
			}
			_ => {}
		}
	}
}
