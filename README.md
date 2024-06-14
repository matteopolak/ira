# Ira

A general-purpose, code-first game engine.

## Examples

### Basic gLTF rendering

```rust
use ira::{winit, App};
use ira_drum::Drum;

fn main() -> Result<(), winit::error::EventLoopError> {
  let app = App::new(|_window| Drum::from_path("car.drum").unwrap());

  app.run()
}
```

More examples can be found in the [`examples`](examples) directory.

