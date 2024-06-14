# Ira

A general-purpose, code-first game engine.

## Examples

### Basic gLTF rendering

```rust
use ira::{glam::Vec3, pollster, winit, App, Instance, Model};
use ira_drum::Drum;

fn main() -> Result<(), winit::error::EventLoopError> {
  let mut drum = DrumBuilder::default();
  let gltf = gltf::Gltf::open("models/bottled_car/scene.gltf").unwrap();

  drum.add_gltf(&gltf, "models/bottled_car".as_ref()).unwrap();

  drum.set_brdf_lut(Texture::from_path("./ibl_brdf_lut.png").unwrap());
  drum.set_irradiance_map(
    Texture::from_path("./ibl_irradiance_map.png")
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

  drum.prepare_textures(|_| Mipmaps::GeneratedAutomatic)
    .unwrap();

  let app = App::new(|_window| drum);

  app.run()
}
```

More examples can be found in the [`examples`](examples) directory.

