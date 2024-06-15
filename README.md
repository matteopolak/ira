# Ira

A general-purpose, code-first game engine.

```rust
struct App;

impl ira::App for App {
  fn init(&mut self, _window: &mut Window) -> Drum {
    Drum::from_path("car.drum").unwrap()
  }

  fn on_ready(&mut self, state: &mut State) {
    let model = &mut state.drum.models[0];

    model.instances.push(Instance::default().with_up(Vec3::Z));
  }

  fn on_frame(&mut self, state: &mut State, delta: Duration) {
    let model = &mut state.drum.models[0];

    // rotate the car model around the Y-axis
    model.update_instance(0, |instance| {
      instance.rotate_y(delta.as_secs_f32() * PI * 0.25);
    });
  }
}

fn main() -> Result<(), EventLoopError> {
  Game::new(App).run()
}
```

More examples can be found in the [`examples`](examples) directory.

## Preparing assets

The `ira` tool can be used to pack various game assets (glTF, etc.) into a single Drum.

See the [ira_cli](ira_cli/README.md) documentation for more information.

