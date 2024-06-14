use ira::{winit, App};
use ira_drum::Drum;

fn main() -> Result<(), winit::error::EventLoopError> {
	let app = App::new(|_window| Drum::from_path("car.drum").unwrap());

	app.run()
}
