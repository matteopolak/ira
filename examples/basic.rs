use ira::{glam::Vec3, winit, App, Model};

fn main() -> Result<(), winit::error::EventLoopError> {
	env_logger::init();

	let app = App::new(|state| {
		pollster::block_on(async {
			let gpu_model = Model::from_path(
				"models/bottled_car/scene.gltf",
				&state.device,
				&state.queue,
				&state.material_bind_group_layout,
				Vec3::Z,
			)
			.await
			.unwrap()
			.into_gpu(&state.device);

			state.models.push(gpu_model);
		});
	});

	app.run()
}
