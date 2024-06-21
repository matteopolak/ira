use glam::Vec3;
use rapier3d::{
	data::Index,
	dynamics::{
		CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
		RigidBodyBuilder, RigidBodyHandle, RigidBodySet,
	},
	geometry::{ColliderBuilder, ColliderHandle, ColliderSet, DefaultBroadPhase, NarrowPhase},
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
	fn add_instance(
		&mut self,
		instance: InstanceBuilder,
		model_id: u32,
		instance_id: u32,
	) -> Instance {
		let mut new_instance = Instance {
			scale: instance.scale,
			collider: None,
			body: Body::Static {
				position: instance.position,
				rotation: instance.rotation,
			},
			model_id,
			instance_id,
		};

		if let Some(body) = instance.rigidbody {
			let body = self.rigid_bodies.insert(body);

			if let Some(collider) = instance.collider {
				new_instance.collider = Some(self.colliders.insert_with_parent(
					collider,
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

pub trait IndexExt {
	fn to_u128(&self) -> u128;
	fn from_u128(id: u128) -> Self;
}

impl IndexExt for Index {
	fn to_u128(&self) -> u128 {
		let (idx, gen) = self.into_raw_parts();

		(idx as u128) << 32 | gen as u128
	}

	fn from_u128(id: u128) -> Self {
		let idx = (id >> 32) as u32;
		let gen = id as u32;

		Self::from_raw_parts(idx, gen)
	}
}

impl<Message> Context<Message> {
	pub fn physics_update(&mut self) {
		self.physics.step();

		for body in self.physics.islands.active_dynamic_bodies() {
			let Some(body) = self.physics.rigid_bodies.get(*body) else {
				continue;
			};

			if body.user_data == u128::MAX {
				continue;
			}

			let instance: InstanceHandle = body.user_data.into();

			instance.resolve_model_mut(&mut self.drum).dirty = true;

			let (gpu, instance) = instance.resolve_mut(&mut self.drum);

			*gpu = instance.to_gpu(&self.physics);
		}
	}

	/// Adds a new rigidbody and collider to the physics world.
	#[allow(clippy::needless_pass_by_value)]
	pub fn add_rigidbody(
		&mut self,
		rigidbody: RigidBodyBuilder,
		collider: ColliderBuilder,
	) -> (RigidBodyHandle, ColliderHandle) {
		let body = self
			.physics
			.rigid_bodies
			.insert(rigidbody.user_data(u128::MAX).build());

		let collider = self.physics.colliders.insert_with_parent(
			collider.build(),
			body,
			&mut self.physics.rigid_bodies,
		);

		(body, collider)
	}

	/// Adds a new collider to the physics world.
	///
	/// To add a collider with a rigidbody, use [`Context::add_rigidbody`] instead.
	#[allow(clippy::needless_pass_by_value)]
	pub fn add_collider(&mut self, collider: ColliderBuilder) -> ColliderHandle {
		self.physics.colliders.insert(collider.build())
	}

	/// Adds a new instance to the drum and physics world.
	///
	/// If you only want to insert a rigidbody with no model,
	/// use [`Context::add_rigidbody`] or [`Context::add_collider`] instead.
	///
	/// When executed on a non-server client, this will do nothing.
	pub fn add_instance(&mut self, model_id: u32, mut instance: InstanceBuilder) -> InstanceHandle {
		let model = &mut self.drum.models[model_id as usize];
		let instance_id = model.instances.len() as u32;
		let index = self.drum.instances.insert_with(|handle| {
			let (axis, angle) = instance.rotation.to_axis_angle();

			match (instance.rigidbody, instance.collider) {
				(Some(body), collider) => {
					instance.collider =
						collider.or_else(|| Some(model.bounds.to_cuboid(instance.scale).mass(1.0)));

					instance.rigidbody = Some(
						body.position(instance.position.into())
							.rotation((axis * angle).into())
							.user_data(handle.to_u128()),
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

			let instance = self.physics.add_instance(instance, model_id, instance_id);

			model.instances.push(instance.to_gpu(&self.physics));

			instance
		});

		InstanceHandle::new(index)
	}
}

#[derive(Debug, Clone, Copy)]
pub struct InstanceHandle {
	handle: Index,
}

impl InstanceHandle {
	#[must_use]
	pub fn new(handle: Index) -> Self {
		Self { handle }
	}

	pub fn resolve<'d>(&self, drum: &'d GpuDrum) -> (&'d GpuInstance, &'d Instance) {
		let instance = drum.instances.get(self.handle).unwrap();

		(
			&drum.models[instance.model_id as usize].instances[instance.instance_id as usize],
			instance,
		)
	}

	/// Returns the instance associated with the handle.
	pub fn resolve_mut<'d>(
		&self,
		drum: &'d mut GpuDrum,
	) -> (&'d mut GpuInstance, &'d mut Instance) {
		let instance = drum.instances.get_mut(self.handle).unwrap();

		(
			&mut drum.models[instance.model_id as usize].instances[instance.instance_id as usize],
			instance,
		)
	}

	/// Returns the model associated with the instance.
	#[must_use]
	pub fn resolve_model<'d>(&self, drum: &'d GpuDrum) -> &'d GpuModel {
		let instance = drum.instances.get(self.handle).unwrap();

		&drum.models[instance.model_id as usize]
	}

	/// Returns the model associated with the instance.
	pub fn resolve_model_mut<'d>(&self, drum: &'d mut GpuDrum) -> &'d mut GpuModel {
		let instance = drum.instances.get(self.handle).unwrap();

		&mut drum.models[instance.model_id as usize]
	}

	/// Updates the instance with the provided closure.
	///
	/// # Examples
	///
	/// ```rust
	/// let instance: InstanceHandle = ...;
	///
	/// instance.update(ctx, |instance, physics| {
	///   instance.rotate_y(physics, 0.1);
	/// });
	/// ```
	pub fn update<F, Message>(&self, ctx: &mut Context<Message>, update: F)
	where
		F: FnOnce(&mut Instance, &mut PhysicsState),
	{
		let (gpu, instance) = self.resolve_mut(&mut ctx.drum);
		let model_id = instance.model_id as usize;

		update(instance, &mut ctx.physics);
		*gpu = instance.to_gpu(&ctx.physics);

		ctx.drum.models[model_id].dirty = true;
	}
}

impl From<u128> for InstanceHandle {
	fn from(id: u128) -> Self {
		Self {
			handle: Index::from_u128(id),
		}
	}
}

impl From<InstanceHandle> for u128 {
	fn from(id: InstanceHandle) -> u128 {
		id.handle.to_u128()
	}
}
