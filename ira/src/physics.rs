use std::time::Duration;

use glam::Vec3;

use crate::{game::State, GpuDrum, GpuInstance, GpuModel, Instance};

#[derive(Debug, Clone, Copy)]
pub enum BoundingBox {
	Cuboid { min: Vec3, max: Vec3 },
	Sphere { center: Vec3, radius: f32 },
}

impl BoundingBox {
	#[must_use]
	pub fn sweep(&self, other: &Self, velocity: Vec3) -> Option<f32> {
		match (*self, *other) {
			(
				BoundingBox::Cuboid {
					min: min_a,
					max: max_a,
				},
				BoundingBox::Cuboid {
					min: min_b,
					max: max_b,
				},
			) => {
				// AABB vs AABB sweep
				let relative_velocity = velocity;
				let mut t_min = -0.2f32;
				let mut t_max = 0.01f32;

				for i in 0..3 {
					if relative_velocity[i] == 0.0 {
						if min_a[i] > max_b[i] || max_a[i] < min_b[i] {
							return None;
						}
					} else {
						let inv = 1.0 / relative_velocity[i];

						let mut t0 = (min_b[i] - max_a[i]) * inv;
						let mut t1 = (max_b[i] - min_a[i]) * inv;

						if t0 > t1 {
							std::mem::swap(&mut t0, &mut t1);
						}

						t_min = t_min.max(t0);
						t_max = t_max.min(t1);

						if t_min > t_max {
							return None;
						}
					}
				}

				Some(t_min)
			}
			(
				BoundingBox::Sphere {
					center: center_a,
					radius: radius_a,
				},
				BoundingBox::Sphere {
					center: center_b,
					radius: radius_b,
				},
			) => {
				// Sphere vs Sphere sweep
				let radius = radius_a + radius_b;
				let relative_velocity = velocity;
				let distance = center_a - center_b;

				let a = relative_velocity.length_squared();
				let b = 2.0 * relative_velocity.dot(distance);
				let c = distance.length_squared() - radius.powi(2);

				let discriminant = b.powi(2) - 4.0 * a * c;

				if discriminant < 0.0 {
					return None;
				}

				let t = (-b - discriminant.sqrt()) / (2.0 * a);

				if t < 0.0 {
					return None;
				}

				Some(t)
			}
			(BoundingBox::Cuboid { min, max }, BoundingBox::Sphere { center, radius })
			| (BoundingBox::Sphere { center, radius }, BoundingBox::Cuboid { min, max }) => {
				// AABB vs Sphere sweep
				let closest_point = Vec3::new(
					center.x.clamp(min.x, max.x),
					center.y.clamp(min.y, max.y),
					center.z.clamp(min.z, max.z),
				);

				let distance = center - closest_point;
				let distance_squared = distance.length_squared();

				if distance_squared < radius.powi(2) {
					return None;
				}

				let relative_velocity = velocity;
				let a = relative_velocity.length_squared();
				let b = 2.0 * relative_velocity.dot(distance);
				let c = distance_squared - radius.powi(2);

				let discriminant = b.powi(2) - 4.0 * a * c;

				if discriminant < 0.0 {
					return None;
				}

				let t = (-b - discriminant.sqrt()) / (2.0 * a);

				if t < 0.0 {
					return None;
				}

				Some(t)
			}
		}
	}

	#[must_use]
	pub fn check_collision(&self, other: &Self) -> bool {
		match (self, other) {
			(
				BoundingBox::Cuboid {
					min: min_a,
					max: max_a,
				},
				BoundingBox::Cuboid {
					min: min_b,
					max: max_b,
				},
			) => {
				// AABB vs AABB collision
				(min_a.x <= max_b.x && max_a.x >= min_b.x)
					&& (min_a.y <= max_b.y && max_a.y >= min_b.y)
					&& (min_a.z <= max_b.z && max_a.z >= min_b.z)
			}
			(
				BoundingBox::Sphere {
					center: center_a,
					radius: radius_a,
				},
				BoundingBox::Sphere {
					center: center_b,
					radius: radius_b,
				},
			) => {
				// Sphere vs Sphere collision
				let distance_squared = center_a.distance_squared(*center_b);
				distance_squared <= (radius_a + radius_b).powi(2)
			}
			(BoundingBox::Cuboid { min, max }, BoundingBox::Sphere { center, radius })
			| (BoundingBox::Sphere { center, radius }, BoundingBox::Cuboid { min, max }) => {
				// AABB vs Sphere collision
				let closest_point = Vec3::new(
					center.x.clamp(min.x, max.x),
					center.y.clamp(min.y, max.y),
					center.z.clamp(min.z, max.z),
				);

				let distance_squared = closest_point.distance_squared(*center);
				distance_squared <= radius.powi(2)
			}
		}
	}

	#[must_use]
	pub fn to_world_space(&self, transform: glam::Mat4) -> Self {
		match self {
			Self::Cuboid { min, max } => {
				let min = transform.transform_point3(*min);
				let max = transform.transform_point3(*max);

				Self::Cuboid { min, max }
			}
			Self::Sphere { center, radius } => {
				let center = transform.transform_point3(*center);

				Self::Sphere {
					center,
					radius: *radius,
				}
			}
		}
	}

	#[must_use]
	pub fn radius(&self) -> f32 {
		match *self {
			Self::Cuboid { min, max } => {
				let half = (max - min) / 2.0;
				half.length()
			}
			Self::Sphere { radius, .. } => radius,
		}
	}
}

pub trait Physics {
	fn update_physics(&mut self, delta: Duration);
}

#[derive(Debug, Clone, Copy)]
pub struct InstanceRef {
	model: usize,
	instance: usize,
}

impl InstanceRef {
	pub fn new(model: usize, instance: usize) -> Self {
		Self { model, instance }
	}

	pub fn resolve<'d>(&self, drum: &'d GpuDrum) -> (&'d GpuInstance, &'d Instance) {
		let model = &drum.models[self.model];

		(
			&model.instances[self.instance],
			&model.instance_data[self.instance],
		)
	}
}

fn broad_phase(models: &[GpuModel]) -> impl Iterator<Item = (InstanceRef, InstanceRef)> + '_ {
	// for now, check everyting with everything after it
	models.iter().enumerate().flat_map(move |(i, a)| {
		a.instances.iter().enumerate().flat_map(move |(j, _x)| {
			models[i + 1..].iter().enumerate().flat_map(move |(k, b)| {
				let k = i + 1 + k;
				let instances = if i == k { j + 1 } else { 0 };

				(b.instances[instances..])
					.iter()
					.enumerate()
					.filter_map(move |(l, _y)| {
						let l = instances + l;

						// if the first doesn't have velocity, skip it
						if a.instance_data[j].velocity.length_squared() == 0.0 {
							return None;
						}

						Some((
							InstanceRef {
								model: i,
								instance: j,
							},
							InstanceRef {
								model: k,
								instance: l,
							},
						))
					})
			})
		})
	})
}

fn get_two_slice<T>(slice: &mut [T], ai: usize, bi: usize) -> (&mut T, &mut T) {
	if ai < bi {
		let (a, b) = slice.split_at_mut(bi);
		(&mut a[ai], &mut b[0])
	} else {
		let (b, a) = slice.split_at_mut(ai);
		(&mut a[0], &mut b[bi])
	}
}

fn get_mut<'m>(
	slice: &'m mut [GpuModel],
	ai: &InstanceRef,
	bi: &InstanceRef,
) -> (
	(&'m mut GpuInstance, &'m mut GpuInstance),
	(&'m mut Instance, &'m mut Instance),
) {
	if ai.model == bi.model {
		let model = &mut slice[ai.model];

		(
			get_two_slice(&mut model.instances, ai.instance, bi.instance),
			get_two_slice(&mut model.instance_data, ai.instance, bi.instance),
		)
	} else {
		let (a, b) = get_two_slice(slice, ai.model, bi.model);

		(
			(&mut a.instances[ai.instance], &mut b.instances[bi.instance]),
			(
				&mut a.instance_data[ai.instance],
				&mut b.instance_data[bi.instance],
			),
		)
	}
}

impl Physics for State {
	fn update_physics(&mut self, delta: Duration) {
		for model in &mut self.drum.models {
			model.update_physics(delta);
		}

		let broad = broad_phase(&self.drum.models).collect::<Vec<_>>();

		for (a, b) in broad {
			let bounding_a = self.drum.models[a.model].bounds;
			let bounding_b = self.drum.models[b.model].bounds;

			let ((g_a, g_b), (a, b)) = get_mut(&mut self.drum.models, &a, &b);

			let ws_a = bounding_a.to_world_space(g_a.model());
			let ws_b = bounding_b.to_world_space(g_b.model());

			if let Some(t) = ws_a.sweep(&ws_b, a.velocity - b.velocity) {
				let t = t.min(delta.as_secs_f32());

				let position_a = a.position + a.velocity * t;
				let position_b = b.position + b.velocity * t;

				let normal = (position_a - position_b).normalize();

				let v_a = a.velocity - normal * a.velocity.dot(normal);
				let v_b = b.velocity - normal * b.velocity.dot(normal);

				a.velocity = v_b + normal * v_a.dot(normal);
				b.velocity = v_a + normal * v_b.dot(normal);

				a.position = position_a;
				b.position = position_b;
			}
		}

		// update all gpu instances
		for model in &mut self.drum.models {
			for (instance, gpu_instance) in
				model.instance_data.iter().zip(model.instances.iter_mut())
			{
				*gpu_instance = instance.to_gpu();
			}
		}
	}
}

impl Physics for GpuModel {
	fn update_physics(&mut self, delta: Duration) {
		for instance in &mut self.instance_data {
			instance.last_position = instance.position;

			if instance.gravity {
				instance.velocity.y -= 9.81 * delta.as_secs_f32();
			}

			instance.position += instance.velocity * delta.as_secs_f32();
		}
	}
}
