use crate::{
	camera, light,
	packet::{Packet, TrustedPacket},
	physics::{InstanceHandle, PhysicsState},
	render, server, Camera, DrumExt, GpuDrum, GpuTexture, InstanceBuilder, MaterialExt,
};

use std::{
	collections::BTreeMap,
	fmt,
	sync::{atomic::AtomicU32, mpsc, Arc},
	time::{self, Duration},
};

use glam::{Vec2, Vec3};
use ira_drum::{Drum, Material};
use winit::{
	application::ApplicationHandler,
	event::{DeviceEvent, KeyEvent, WindowEvent},
	event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
	keyboard::{KeyCode, PhysicalKey},
	window::{CursorGrabMode, Fullscreen, Window, WindowAttributes, WindowId},
};

/// Implemented by the application to handle user input,
/// physics, and rendering.
///
/// For networked games, implement the [`Network`] trait as well.
#[allow(unused_variables)]
pub trait App<Message = ()> {
	/// Connects to the server. This is only called on clients.
	#[must_use]
	fn connect() -> std::net::TcpStream {
		std::net::TcpStream::connect("127.0.0.1:12345").unwrap()
	}

	/// Creates a new listener for the server to receive connections.
	#[must_use]
	fn listen() -> std::net::TcpListener {
		std::net::TcpListener::bind("127.0.0.1:12345").unwrap()
	}

	/// Called when a remote player joins the game. This should return
	/// the model id, and a builder for the instance.
	fn create_player(ctx: &mut Context<Message>) -> (u32, InstanceBuilder);

	/// Called when a custom message is received from the server.
	fn on_message(ctx: &mut Context<Message>, message: Message) {}
	/// Called once at the start of the program, right after the window
	/// is created but before anything else is done.
	fn on_init() -> Drum;
	/// Called once when everything has been created on the GPU.
	fn on_ready(ctx: &mut Context<Message>) -> Self;
	/// Called once per frame, right before rendering. Note that this
	/// will not be called if the `render` feature is not enabled.
	fn on_update(&mut self, ctx: &mut Context<Message>, delta: Duration) {}
	/// Called every 1/60th of a second. If queued at the same time as an update,
	/// this will always be called first.
	fn on_fixed_update(&mut self, ctx: &mut Context<Message>) {}
}

/// A game instance.
///
/// [`A`] is the application type (that implements [`App`]).
/// [`Message`] is the custom message that can be sent between the client and server.
pub struct Game<A, Message = ()> {
	state: Option<(Context<Message>, A)>,
}

impl<A, Message> Default for Game<A, Message> {
	fn default() -> Self {
		Self { state: None }
	}
}

impl<A: App<Message>, Message> Game<A, Message> {
	/// Runs the application.
	///
	/// This function will block the current thread until the application is closed.
	///
	/// # Errors
	///
	/// See [`winit::error::EventLoopError`] for errors.
	pub fn run(mut self) -> Result<(), winit::error::EventLoopError>
	where
		Message: bitcode::Encode + bitcode::DecodeOwned + fmt::Debug + Send + 'static,
	{
		let event_loop = EventLoop::new()?;

		event_loop.set_control_flow(ControlFlow::Poll);
		event_loop.run_app(&mut self)
	}
}

/// The game context, containing all game-related data.
pub struct Context<Message = ()> {
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

	// instance_id -> instance
	pub(crate) handles: BTreeMap<u32, InstanceHandle>,
	// instance -> instance_id
	pub(crate) instances: BTreeMap<InstanceHandle, u32>,
	// client_id -> instance_id
	pub(crate) clients: BTreeMap<u32, u32>,

	/// Used to send messages to the thread that communicates with the server.
	/// If the "server" feature is enabled, this will be directly to the server.
	pub(crate) packet_tx: mpsc::Sender<Packet<Message>>,
	/// Used to receive messages from the thread that communicates with the server.
	/// If the "server" feature is enabled, this will be directly from the server.
	pub(crate) packet_rx: mpsc::Receiver<TrustedPacket<Message>>,

	pub(crate) next_instance_id: Arc<AtomicU32>,
}

impl<Message> Context<Message> {
	async fn new<A: App<Message>>(window: Window, drum: Drum) -> Self
	where
		Message: bitcode::Encode + bitcode::DecodeOwned + fmt::Debug + Send + 'static,
	{
		let window = Arc::new(window);
		let size = window.inner_size();
		let instance = wgpu::Instance::default();

		let surface = instance.create_surface(window.clone()).unwrap();
		let adapter = render::request_adapter(&instance, &surface).await;

		let (device, queue) = render::request_device(&adapter).await;

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
		let drum = drum.into_gpu(&device, &queue);

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

		let (opaque_render_pipeline, transparent_render_pipeline) =
			render::create_pbr_render_pipelines(
				&device,
				&material_bind_group_layout,
				&brdf_bind_group_layout,
				&camera,
				&lights,
				sample_count,
			);

		let multisampled_texture = GpuTexture::create_multisampled(&device, &config, sample_count);

		let (server_packet_tx, packet_rx) = mpsc::channel();
		let (packet_tx, server_packet_rx) = mpsc::channel();

		let next_instance_id = Arc::new(AtomicU32::new(0));

		std::thread::spawn({
			let next_instance_id = Arc::clone(&next_instance_id);

			move || {
				server::run::<A, _>(server_packet_tx, server_packet_rx, next_instance_id);
			}
		});

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

			handles: BTreeMap::new(),
			instances: BTreeMap::new(),
			clients: BTreeMap::new(),

			packet_tx,
			packet_rx,

			next_instance_id,
		}
	}

	pub fn pressed(&self, key: KeyCode) -> bool {
		self.pressed_keys.contains(&key)
	}

	pub fn just_pressed(&self, key: KeyCode) -> bool {
		self.just_pressed.contains(&key)
	}

	pub fn mouse_delta(&self) -> Vec2 {
		self.mouse_delta
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

impl<A: App<Message>, Message> Game<A, Message> {
	fn on_packet(ctx: &mut Context<Message>, packet: TrustedPacket<Message>) {
		let client_id = packet.client_id;

		match packet.inner {
			Packet::Custom(message) => A::on_message(ctx, message),
			Packet::CreateClient { instance_id } => {
				let (model_id, instance) = A::create_player(ctx);
				let instance = ctx.add_instance_local(model_id, instance);

				ctx.handles.insert(instance_id, instance);
				ctx.instances.insert(instance, instance_id);
			}
			Packet::DeleteClient => {
				let Some(instance_id) = ctx.clients.remove(&client_id) else {
					return;
				};

				let Some(handle) = ctx.handles.remove(&instance_id) else {
					return;
				};

				ctx.instances.remove(&handle);
			}
			Packet::CreateInstance { options, id } => {
				let instance = ctx.add_instance_local(options.model_id, options.into_builder());

				ctx.handles.insert(id, instance);
				ctx.instances.insert(instance, id);
			}
			Packet::DeleteInstance { id } => {
				ctx.remove_instance_local(id);
			}
			Packet::UpdateInstance { id, delta } => {
				let Some(&mut instance) = ctx.handles.get_mut(&id) else {
					return;
				};

				instance.update(ctx, |i, p| {
					delta.apply(p, i);
				});
			}
		}
	}

	fn render(&mut self, event_loop: &ActiveEventLoop) {
		let Some((ctx, app)) = self.state.as_mut() else {
			return;
		};

		while let Ok(packet) = ctx.packet_rx.try_recv() {
			Self::on_packet(ctx, packet);
		}

		let delta = ctx.last_physics.elapsed();

		// physics 60fps
		if delta.as_secs_f32() >= 1.0 / 60.0 {
			ctx.last_physics = time::Instant::now();
			ctx.physics_update();
			app.on_fixed_update(ctx);
		}

		#[cfg(feature = "client")]
		{
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
}

impl<A: App<Message>, Message> ApplicationHandler for Game<A, Message>
where
	Message: bitcode::Encode + bitcode::DecodeOwned + fmt::Debug + Send + 'static,
{
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let window = event_loop
			.create_window(WindowAttributes::default().with_title("Triangle"))
			.unwrap();

		window.set_cursor_grab(CursorGrabMode::Locked).unwrap();
		window.set_cursor_visible(false);

		let drum = A::on_init();
		let mut ctx = pollster::block_on(Context::<Message>::new::<A>(window, drum));
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
