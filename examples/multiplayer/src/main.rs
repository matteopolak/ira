#![allow(dead_code, unused_variables, unused_imports)]

use std::{
	f32::consts::FRAC_PI_4,
	time::{Duration, Instant},
};

use ira::{
	extra::ratelimit::Ratelimit,
	glam::{Quat, Vec3},
	packet::{Packet, TrustedPacket},
	physics::InstanceHandle,
	ColliderBuilder, Context, Game, Instance, KeyCode, RigidBodyBuilder,
};
use ira_drum::Drum;

struct App {
	#[cfg(feature = "client")]
	player: basic::Player,
	cars: Vec<InstanceHandle>,

	car_spawn: Ratelimit,
	hello: Ratelimit,
}

#[derive(Debug, bitcode::Encode, bitcode::Decode)]
enum Message {
	Hello,
}

impl ira::App<Message> for App {
	fn listen() -> std::net::TcpListener {
		std::net::TcpListener::bind("0.0.0.0:10585").expect("failed to bind to port 10585")
	}

	fn connect() -> std::net::TcpStream {
		std::net::TcpStream::connect(
			std::env::args()
				.nth(1)
				.expect("expected an IP address to connect to as the first argument"),
		)
		.expect("failed to connect to server")
	}

	fn create_player(ctx: &mut Context<Message>) -> (u32, ira::InstanceBuilder) {
		(
			2,
			ira::Instance::builder()
				.rigidbody(RigidBodyBuilder::dynamic().lock_rotations())
				.scale(Vec3::splat(0.05))
				.collider(
					ctx.drum
						.model_by_name("orb")
						.unwrap()
						.bounds()
						.to_cuboid(Vec3::splat(0.05))
						.mass(0.5),
				),
		)
	}

	fn on_init() -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_packet(ctx: &mut Context<Message>, packet: TrustedPacket<Message>) {
		let client_id = packet.client_id;
		let Packet::Custom(message) = packet.inner else {
			return;
		};

		match message {
			Message::Hello => {
				println!("Hello from {client_id:?}");
			}
		}
	}

	fn on_ready(ctx: &mut Context<Message>) -> Self {
		let car_id = ctx.drum.model_id("bottled_car").unwrap();

		#[cfg(feature = "server")]
		let cars = (0..0)
			.map(|i| {
				ctx.add_instance(
					car_id,
					Instance::builder()
						.up(Vec3::Z)
						.rotation(Quat::from_rotation_x(FRAC_PI_4))
						.position(Vec3::new(0.0, 10.0 + i as f32 * 5.0, 0.0))
						.rigidbody(RigidBodyBuilder::dynamic()),
				)
			})
			.collect();
		#[cfg(not(feature = "server"))]
		let cars = Vec::new();

		#[cfg(feature = "server")]
		ctx.add_instance(
			ctx.drum.model_id("boring_cube").unwrap(),
			Instance::builder()
				.position(Vec3::Y * -10.0)
				.scale(Vec3::new(10.0, 1.0, 10.0)),
		);

		#[cfg(feature = "client")]
		let body_collider = ctx.add_rigidbody(
			RigidBodyBuilder::dynamic().lock_rotations(),
			ctx.drum
				.model_by_name("orb")
				.unwrap()
				.bounds()
				.to_cuboid(Vec3::splat(0.05))
				.mass(0.5),
		);

		Self {
			#[cfg(feature = "client")]
			player: basic::Player::new(Instance::from(body_collider)),
			cars,
			car_spawn: Ratelimit::new(Duration::from_secs(1)),
			hello: Ratelimit::new(Duration::from_millis(500)),
		}
	}

	fn on_fixed_update(&mut self, ctx: &mut Context<Message>) {
		#[cfg(feature = "client")]
		self.player.on_fixed_update(ctx);
	}

	fn on_update(&mut self, ctx: &mut Context<Message>, delta: Duration) {
		#[cfg(feature = "client")]
		self.player.on_update(ctx, delta);

		#[cfg(feature = "client")]
		if ctx.pressed(ira::KeyCode::KeyC) && self.car_spawn.check() {
			let car_id = ctx.drum.model_id("bottled_car").unwrap();
			let handle = ctx.add_instance(
				car_id,
				Instance::builder()
					.up(Vec3::Z)
					.rotation(Quat::from_rotation_x(FRAC_PI_4))
					.position(Vec3::new(0.0, 10.0, 0.0))
					.scale(Vec3::splat(10.0))
					.rigidbody(RigidBodyBuilder::dynamic()),
			);

			#[cfg(feature = "server")]
			self.cars.push(handle);
		}

		#[cfg(feature = "client")]
		if ctx.pressed(ira::KeyCode::KeyH) && self.hello.check() {
			ctx.send_packet(Packet::new(Message::Hello));
		}
	}
}

fn main() -> Result<(), ira::game::Error> {
	tracing_subscriber::fmt::init();

	Game::<App, Message>::default().run()
}
