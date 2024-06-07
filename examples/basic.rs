use ira::{glam::Vec3, pollster, winit, App, Instance, Model, Texture};

fn main() -> Result<(), winit::error::EventLoopError> {
	env_logger::init();

	let app = App::new(|state| {
		pollster::block_on(async {
			let cubemap =
				Texture::try_from_path(&state.device, &state.queue, "./yokohama.jpg", "cubemap")
					.expect("failed to load cubemap");

			let gpu_model = Model::from_path(
				"models/bottled_car/scene.gltf",
				&state.device,
				&state.queue,
				&state.material_bind_group_layout,
				&cubemap,
			)
			.await
			.unwrap()
			.with_instance(Instance::from_up(Vec3::ZERO, Vec3::Z))
			.into_gpu(&state.device);

			state.models.push(gpu_model);
		});

		state.controller.camera.recreate_bind_group(&state.device);
	});

	app.run()
}
