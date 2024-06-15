use std::time::Duration;

use crate::{game::State, GpuModel};

pub trait Physics {
	fn update_physics(&mut self, delta: Duration);
}

impl Physics for State {
	fn update_physics(&mut self, delta: Duration) {
		for model in &mut self.drum.models {
			model.update_physics(delta);
		}
	}
}

impl Physics for GpuModel {
	fn update_physics(&mut self, delta: Duration) {
		for (instance, gpu) in self.instance_data.iter_mut().zip(self.instances.iter_mut()) {
			if instance.gravity {
				instance.velocity.y -= 9.81 * delta.as_secs_f32() / 10.0;
			}

			instance.position += instance.velocity * delta.as_secs_f32();
			*gpu = instance.to_gpu();
		}
	}
}
