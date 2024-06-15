use std::{f32::consts::PI, time::Duration};

use ira::{
	game::State,
	glam::Vec3,
	winit::{error::EventLoopError, window::Window},
	Game, Instance,
};
use ira_drum::Drum;

struct App {
	instance: Instance,
}

impl Default for App {
	fn default() -> Self {
		Self {
			instance: Instance::from_up(Vec3::ZERO, Vec3::Z),
		}
	}
}

impl ira::App for App {
	fn on_ready(&mut self, _window: &mut Window) -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_frame(&mut self, state: &mut State, delta: Duration) {
		let model = &mut state.drum.models[0];

		self.instance.rotate_y(delta.as_secs_f32() * PI * 0.25);

		model.set_instance(0, &self.instance);
		model.recreate_instance_buffer(&state.device);
	}
}

fn main() -> Result<(), EventLoopError> {
	let app = Game::new(App::default());

	app.run()
}
