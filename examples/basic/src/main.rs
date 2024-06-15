use std::{f32::consts::PI, time::Duration};

use ira::{
	game::State,
	glam::Vec3,
	winit::{error::EventLoopError, window::Window},
	Game, Instance,
};
use ira_drum::Drum;

struct App;

impl ira::App for App {
	fn init(&mut self, _window: &mut Window) -> Drum {
		Drum::from_path("car.drum").unwrap()
	}

	fn on_ready(&mut self, state: &mut State) {
		// car
		let car = &mut state.drum.models[0];

		car.add_instance(
			Instance::default()
				.with_up(Vec3::Z)
				.with_position(Vec3::Y * 200.0)
				.with_gravity(),
		);

		// floor
		let floor = &mut state.drum.models[1];

		floor.add_instance(Instance::default());
	}

	fn on_frame(&mut self, state: &mut State, delta: Duration) {
		let model = &mut state.drum.models[0];

		model.update_instance(0, |instance| {
			instance.rotate_y(delta.as_secs_f32() * PI * 0.25);
		});
	}
}

fn main() -> Result<(), EventLoopError> {
	Game::new(App).run()
}
