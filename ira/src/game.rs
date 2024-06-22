use crate::{
	client::ClientId,
	packet::{CreateInstance, Packet, TrustedPacket},
	physics::{InstanceHandle, PhysicsState},
	render::RenderState,
	server::{self, InstanceId},
	DrumExt, GpuDrum, Instance, InstanceBuilder,
};

use std::{
	collections::BTreeMap,
	fmt,
	sync::{atomic::AtomicU32, mpsc, Arc},
	time::{self, Duration},
};

use glam::Vec2;
use ira_drum::Drum;
use rapier3d::data::Arena;
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
	last_frame: time::Instant,
	last_physics: time::Instant,

	pub physics: PhysicsState,

	pub(crate) desired_fps: f32,

	pub(crate) pressed_keys: Vec<KeyCode>,
	pub(crate) just_pressed: Vec<KeyCode>,
	pub(crate) mouse_delta: Vec2,

	pub(crate) handles: BTreeMap<InstanceId, InstanceHandle>,
	pub(crate) instance_ids: BTreeMap<InstanceHandle, InstanceId>,
	pub(crate) clients: BTreeMap<ClientId, InstanceId>,

	pub instances: Arena<Instance>,

	/// Used to send messages to the thread that communicates with the server.
	/// If the "server" feature is enabled, this will be directly to the server.
	pub(crate) packet_tx: mpsc::Sender<Packet<Message>>,
	/// Used to receive messages from the thread that communicates with the server.
	/// If the "server" feature is enabled, this will be directly from the server.
	pub(crate) packet_rx: mpsc::Receiver<TrustedPacket<Message>>,

	pub(crate) next_instance_id: Arc<AtomicU32>,
	pub client_id: Option<ClientId>,
	pub instance_id: Option<InstanceId>,

	pub drum: GpuDrum,
	#[cfg(feature = "client")]
	pub render: RenderState,
}

impl<Message> Context<Message> {
	#[allow(clippy::too_many_lines)]
	async fn new<A: App<Message>>(window: Window, drum: Drum) -> Self
	where
		Message: bitcode::Encode + bitcode::DecodeOwned + fmt::Debug + Send + 'static,
	{
		#[cfg(feature = "client")]
		let render = RenderState::new(window, &drum).await;
		let physics = PhysicsState::default();

		let (server_packet_tx, packet_rx) = mpsc::channel();
		let (packet_tx, server_packet_rx) = mpsc::channel();

		let next_instance_id = Arc::new(AtomicU32::new(0));

		let drum = drum.into_gpu(
			#[cfg(feature = "client")]
			&render.device,
			#[cfg(feature = "client")]
			&render.queue,
		);

		let mut ctx = Self {
			#[cfg(feature = "client")]
			render,
			drum,
			last_frame: time::Instant::now(),
			last_physics: time::Instant::now(),
			physics,

			desired_fps: 120.0,

			pressed_keys: Vec::new(),
			just_pressed: Vec::new(),
			mouse_delta: Vec2::ZERO,

			handles: BTreeMap::new(),
			instance_ids: BTreeMap::new(),
			instances: Arena::new(),
			clients: BTreeMap::new(),

			packet_tx,
			packet_rx,

			next_instance_id: Arc::clone(&next_instance_id),
			client_id: None,
			instance_id: None,
		};

		std::thread::spawn({
			let (model_id, builder) = A::create_player(&mut ctx);

			move || {
				server::run::<A, _>(
					server_packet_tx,
					server_packet_rx,
					next_instance_id,
					CreateInstance::from_builder(&builder, model_id),
				);
			}
		});

		ctx
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
				ctx.instance_ids.insert(instance, instance_id);
				ctx.clients.insert(client_id, instance_id);
			}
			Packet::DeleteClient => {
				let Some(instance_id) = ctx.clients.remove(&client_id) else {
					return;
				};

				let Some(handle) = ctx.handles.remove(&instance_id) else {
					return;
				};

				ctx.instance_ids.remove(&handle);
			}
			Packet::CreateInstance { options, id } => {
				if Some(id) == ctx.instance_id {
					return;
				}

				let instance = ctx.add_instance_local(options.model_id, options.into_builder());

				ctx.handles.insert(id, instance);
				ctx.instance_ids.insert(instance, id);
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
			Packet::Connected { instance_id } => {
				ctx.client_id = Some(client_id);
				ctx.instance_id = Some(instance_id);
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
			#[cfg(feature = "client")]
			ctx.render.camera.update_view_proj(&ctx.render.queue);

			ctx.just_pressed.clear();
			ctx.mouse_delta = Vec2::ZERO;

			#[cfg(feature = "client")]
			{
				for model in &mut ctx.drum.models {
					model.update_instance_buffer(&ctx.render.device, &ctx.render.queue);
				}

				match ctx.render.render_frame(&ctx.drum) {
					Ok(..) => {}
					Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
						ctx.render.resize(ctx.render.window.inner_size());
					}
					Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
					Err(wgpu::SurfaceError::Timeout) => log::warn!("surface timeout"),
				}
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
			.create_window(WindowAttributes::default().with_title(
				#[cfg(feature = "server")]
				"Server",
				#[cfg(not(feature = "server"))]
				"Client",
			))
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

		if window_id != ctx.render.window.id() {
			return;
		}

		match event {
			WindowEvent::Resized(size) => {
				ctx.render.resize(size);
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
						KeyCode::F11 if ctx.render.window.fullscreen().is_none() => {
							ctx.render
								.window
								.set_fullscreen(Some(Fullscreen::Borderless(None)));
						}
						KeyCode::F11 => {
							ctx.render.window.set_fullscreen(None);
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
