use ira::{winit, App};

fn main() -> Result<(), winit::error::EventLoopError> {
	let app = App::new(|_window| ira_drum::Drum::from_path("./bottled_car.drum").unwrap());

	app.run()
}
