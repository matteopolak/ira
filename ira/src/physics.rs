use glam::Vec3;
use rapier3d::{
	dynamics::{
		CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
		RigidBodySet,
	},
	geometry::{ColliderBuilder, ColliderSet, DefaultBroadPhase, NarrowPhase},
	pipeline::PhysicsPipeline,
};

use crate::{game::Context, Body, GpuDrum, GpuInstance, GpuModel, Instance, InstanceBuilder};

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
	pub min: Vec3,
	pub max: Vec3,
}

impl BoundingBox {
	pub fn to_cuboid(&self, scale: Vec3) -> ColliderBuilder {
		ColliderBuilder::cuboid(
			(self.max.x - self.min.x) * 0.5 * scale.x,
			(self.max.y - self.min.y) * 0.5 * scale.y,
			(self.max.z - self.min.z) * 0.5 * scale.z,
		)
	}
}

pub struct PhysicsState {
	pub pipeline: PhysicsPipeline,

	pub integration: IntegrationParameters,
	pub islands: IslandManager,
	pub broad_phase: DefaultBroadPhase,
	pub narrow_phase: NarrowPhase,

	pub impulse_joins: ImpulseJointSet,
	pub multibody_joints: MultibodyJointSet,

	pub ccd_solver: CCDSolver,

	pub rigid_bodies: RigidBodySet,
	pub colliders: ColliderSet,
}

impl Default for PhysicsState {
	fn default() -> Self {
		Self {
			pipeline: PhysicsPipeline::new(),
			integration: IntegrationParameters::default(),
			islands: IslandManager::new(),
			broad_phase: DefaultBroadPhase::new(),
			narrow_phase: NarrowPhase::new(),
			impulse_joins: ImpulseJointSet::new(),
			multibody_joints: MultibodyJointSet::new(),
			ccd_solver: CCDSolver::new(),
			rigid_bodies: RigidBodySet::new(),
			colliders: ColliderSet::new(),
		}
	}
}

impl PhysicsState {
	pub const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);

	pub fn step(&mut self) {
		self.pipeline.step(
			&nalgebra::Vector3::from(Self::GRAVITY),
			&self.integration,
			&mut self.islands,
			&mut self.broad_phase,
			&mut self.narrow_phase,
			&mut self.rigid_bodies,
			&mut self.colliders,
			&mut self.impulse_joins,
			&mut self.multibody_joints,
			&mut self.ccd_solver,
			None,
			&(),
			&(),
		);
	}

	/// Adds a new instance to the physics world.
	///
	/// Modifies the provided instance to include the rigid body and collider handles.
	fn add_instance(&mut self, instance: InstanceBuilder) -> Instance {
		let mut new_instance = Instance {
			scale: instance.scale,
			collider: None,
			body: Body::Static {
				position: instance.position,
				rotation: instance.rotation,
			},
		};

		if let Some(body) = instance.rigidbody {
			let body = self.rigid_bodies.insert(body);

			if let Some(collider) = instance.collider {
				new_instance.collider = Some(self.colliders.insert_with_parent(
					collider.mass(1.0),
					body,
					&mut self.rigid_bodies,
				));
			}

			new_instance.body = Body::Rigid(body);
		} else if let Some(collider) = instance.collider {
			new_instance.collider = Some(self.colliders.insert(collider.mass(1.0)));
		}

		new_instance
	}
}

impl Context {
	pub fn physics_update(&mut self) {
		self.physics.step();

		for body in self.physics.islands.active_dynamic_bodies() {
			let Some(body) = self.physics.rigid_bodies.get(*body) else {
				continue;
			};

			let instance: InstanceHandle = body.user_data.into();

			instance.resolve_model_mut(&mut self.drum).dirty = true;

			let (gpu, instance) = instance.resolve_mut(&mut self.drum);

			*gpu = instance.to_gpu(&self.physics);
		}
	}

	pub fn add_instance(
		&mut self,
		model_id: usize,
		mut instance: InstanceBuilder,
	) -> InstanceHandle {
		let model = &mut self.drum.models[model_id];
		let handle = InstanceHandle::new(model_id, model.instances.len());

		let (axis, angle) = instance.rotation.to_axis_angle();

		match (instance.rigidbody, instance.collider) {
			(Some(body), collider) => {
				if collider.is_none() {
					instance.collider = Some(model.bounds.to_cuboid(instance.scale));
				} else {
					instance.collider = collider;
				}

				instance.rigidbody = Some(
					body.position(instance.position.into())
						.rotation((axis * angle).into())
						.user_data(handle.into()),
				);
			}
			(body, Some(collider)) => {
				instance.rigidbody = body;
				instance.collider = Some(
					collider
						.position(instance.position.into())
						.rotation((axis * angle).into()),
				);
			}
			(body, None) => {
				instance.rigidbody = body;
				instance.collider = Some(
					model
						.bounds
						.to_cuboid(instance.scale)
						.position(instance.position.into())
						.rotation((axis * angle).into()),
				);
			}
		}

		let instance = self.physics.add_instance(instance);

		model.instances.push(instance.to_gpu(&self.physics));
		model.instance_data.push(instance);

		handle
	}
}

#[derive(Debug, Clone, Copy)]
pub struct InstanceHandle {
	model: usize,
	instance: usize,
}

impl InstanceHandle {
	#[must_use]
	pub fn new(model: usize, instance: usize) -> Self {
		Self { model, instance }
	}

	pub fn resolve<'d>(&self, drum: &'d GpuDrum) -> (&'d GpuInstance, &'d Instance) {
		let model = self.resolve_model(drum);

		(
			&model.instances[self.instance],
			&model.instance_data[self.instance],
		)
	}

	pub fn resolve_mut<'d>(
		&self,
		drum: &'d mut GpuDrum,
	) -> (&'d mut GpuInstance, &'d mut Instance) {
		let model = self.resolve_model_mut(drum);

		(
			&mut model.instances[self.instance],
			&mut model.instance_data[self.instance],
		)
	}

	#[must_use]
	pub fn resolve_model<'d>(&self, drum: &'d GpuDrum) -> &'d GpuModel {
		&drum.models[self.model]
	}

	pub fn resolve_model_mut<'d>(&self, drum: &'d mut GpuDrum) -> &'d mut GpuModel {
		&mut drum.models[self.model]
	}

	pub fn update<F>(&self, ctx: &mut Context, update: F)
	where
		F: FnOnce(&mut Instance, &mut PhysicsState),
	{
		let (gpu, instance) = self.resolve_mut(&mut ctx.drum);

		update(instance, &mut ctx.physics);
		*gpu = instance.to_gpu(&ctx.physics);

		ctx.drum.models[self.model].dirty = true;
	}
}

impl From<u128> for InstanceHandle {
	fn from(id: u128) -> Self {
		Self {
			model: (id >> 64) as usize,
			instance: id as usize,
		}
	}
}

impl From<InstanceHandle> for u128 {
	fn from(id: InstanceHandle) -> u128 {
		(id.model as u128) << 64 | id.instance as u128
	}
}
