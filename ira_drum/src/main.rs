use ira_drum::{self, DrumBuilder, Texture};

fn main() {
	let gltf = gltf::Gltf::open("models/bottled_car/scene.gltf").unwrap();
	let mut drum = DrumBuilder::from_gltf(&gltf, "models/bottled_car".as_ref()).unwrap();

	drum.set_brdf_lut(Texture::from_path("./ibl_brdf_lut.png").unwrap());
	drum.set_irradiance_map(
		Texture::from_path("./ibl_irradiance_map.jpg")
			.unwrap()
			.into_cubemap()
			.unwrap(),
	);
	drum.set_prefiltered_map(
		Texture::from_path("./ibl_prefilter_map.png")
			.unwrap()
			.into_cubemap()
			.unwrap(),
	);

	let mut drum = drum.build();

	drum.compress_textures().unwrap();

	println!("{drum:#?}");

	drum.write_to_path("./car.drum").unwrap();
}
