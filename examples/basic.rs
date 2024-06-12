use ira::{glam::Vec3, pollster, winit, App, Instance, Model, Texture};

fn main() -> Result<(), winit::error::EventLoopError> {
	env_logger::init();

	let app = App::new(|_window| ira_drum::Drum::from_path("./bottled_car.drum").unwrap());

	app.run()
}
