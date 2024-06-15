use std::time::Duration;

use glam::Vec3;

use crate::{game::State, GpuInstance, GpuModel, Instance};

#[derive(Debug, Clone, Copy)]
pub enum BoundingBox {
	Cuboid { min: Vec3, max: Vec3 },
	Sphere { center: Vec3, radius: f32 },
}

impl BoundingBox {
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
struct InstanceRef {
	model: usize,
	instance: usize,
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
					.map(move |(l, _y)| {
						let l = instances + l;

						(
							InstanceRef {
								model: i,
								instance: j,
							},
							InstanceRef {
								model: k,
								instance: l,
							},
						)
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

			if ws_a.check_collision(&ws_b) {
				// collision response
				let normal = (a.position - b.position).normalize();
				let relative_velocity = a.velocity - b.velocity;

				let impulse = normal.dot(relative_velocity) * normal;

				a.velocity -= impulse;
				b.velocity += impulse;

				// use last pos
				a.position = a.last_position;
				b.position = b.last_position;

				// disable gravity
				a.gravity = false;
				b.gravity = false;
			}
		}
	}
}

impl Physics for GpuModel {
	fn update_physics(&mut self, delta: Duration) {
		for (instance, gpu) in self.instance_data.iter_mut().zip(self.instances.iter_mut()) {
			instance.last_position = instance.position;

			if instance.gravity {
				instance.velocity.y -= 9.81 * delta.as_secs_f32() / 10.0;
			}

			if instance.velocity != Vec3::ZERO {
				instance.position += instance.velocity * delta.as_secs_f32();
				*gpu = instance.to_gpu();
			}
		}
	}
}
