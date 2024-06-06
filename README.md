# Ira

A general-purpose, code-first game engine.

## Examples

### Basic gLTF rendering

```rust
use ira::{glam::Vec3, pollster, winit, App, Instance, Model};

fn main() -> Result<(), winit::error::EventLoopError> {
  let app = App::new(|state| {
    pollster::block_on(async {
      let gpu_model = Model::from_path(
        "models/bottled_car/scene.gltf",
        &state.device,
        &state.queue,
        &state.material_bind_group_layout,
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
```

More examples can be found in the [`examples`](examples) directory.

