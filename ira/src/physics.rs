use std::ops;

use glam::Vec3;
use rapier3d::{
	data::{Arena, Index},
	dynamics::{
		CCDSolver, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
		RigidBodyBuilder, RigidBodyHandle, RigidBodySet,
	},
	geometry::{ColliderBuilder, ColliderHandle, ColliderSet, DefaultBroadPhase, NarrowPhase},
	pipeline::PhysicsPipeline,
};

use crate::{
	game::Context,
	packet::{CreateInstance, Packet, UpdateInstance},
	server::InstanceId,
	Body, GpuDrum, GpuModel, Instance, InstanceBuilder,
};

#[cfg(feature = "client")]
use crate::render::model::GpuInstance;

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

	pub impulse_joints: ImpulseJointSet,
	pub multibody_joints: MultibodyJointSet,

	pub ccd_solver: CCDSolver,

	pub rigid_bodies: RigidBodySet,
	pub colliders: ColliderSet,

	pub(crate) steps_since_last_update: u32,
}

impl Default for PhysicsState {
	fn default() -> Self {
		Self {
			pipeline: PhysicsPipeline::new(),
			integration: IntegrationParameters::default(),
			islands: IslandManager::new(),
			broad_phase: DefaultBroadPhase::new(),
			narrow_phase: NarrowPhase::new(),
			impulse_joints: ImpulseJointSet::new(),
			multibody_joints: MultibodyJointSet::new(),
			ccd_solver: CCDSolver::new(),
			rigid_bodies: RigidBodySet::new(),
			colliders: ColliderSet::new(),
			steps_since_last_update: 0,
		}
	}
}

impl PhysicsState {
	pub const GRAVITY: Vec3 = Vec3::new(0.0, -9.81, 0.0);
	pub const TICKS_PER_UPDATE: u32 = 10;

	pub fn step(&mut self) {
		self.pipeline.step(
			&nalgebra::Vector3::from(Self::GRAVITY),
			&self.integration,
			&mut self.islands,
			&mut self.broad_phase,
			&mut self.narrow_phase,
			&mut self.rigid_bodies,
			&mut self.colliders,
			&mut self.impulse_joints,
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
			let (axis, angle) = instance.rotation.to_axis_angle();

			new_instance.collider = Some(
				self.colliders.insert(
					collider
						.position(instance.position.into())
						.rotation((axis * angle).into())
						.mass(1.0),
				),
			);
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

impl<M> Context<M> {
	pub fn physics_update(&mut self) {
		self.physics.step();
		self.physics.steps_since_last_update += 1;

		for body in self
			.physics
			.islands
			.active_dynamic_bodies()
			.iter()
			.chain(self.physics.islands.active_kinematic_bodies())
		{
			let Some(body) = self.physics.rigid_bodies.get(*body) else {
				continue;
			};

			if body.user_data == u128::MAX {
				continue;
			}

			let instance: InstanceHandle = body.user_data.into();

			#[cfg(feature = "client")]
			if let Some(mut pair) = instance.resolve_mut(&mut self.drum, &mut self.instances) {
				pair.update_gpu(&self.physics);
			}

			#[cfg(feature = "server")]
			{
				if self.physics.steps_since_last_update < PhysicsState::TICKS_PER_UPDATE {
					continue;
				}

				let Some(id) = self.instance_ids.get(&instance) else {
					continue;
				};

				let (position, rotation) = (*body.position()).into();
				let _ = self.packet_tx.send(Packet::UpdateInstance {
					id: *id,
					delta: UpdateInstance {
						position,
						rotation,
						scale: None,
						body: None,
					},
				});
			}
		}

		#[cfg(feature = "server")]
		if self.physics.steps_since_last_update >= PhysicsState::TICKS_PER_UPDATE {
			self.physics.steps_since_last_update = 0;
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

	pub fn remove_instance_local(&mut self, id: InstanceId) -> Option<Instance> {
		let instance = self.handles.remove(&id)?;

		self.instance_ids.remove(&instance);

		let instance = self.instances.remove(*instance)?;

		if let Body::Rigid(handle) = instance.body {
			self.physics.rigid_bodies.remove(
				handle,
				&mut self.physics.islands,
				&mut self.physics.colliders,
				&mut self.physics.impulse_joints,
				&mut self.physics.multibody_joints,
				true,
			);
		} else if let Some(collider) = instance.collider {
			self.physics.colliders.remove(
				collider,
				&mut self.physics.islands,
				&mut self.physics.rigid_bodies,
				true,
			);
		}

		// swap_remove the gpu instance, then update the other instance pointing to the one at the end
		let model = &mut self.drum.models[instance.model_id as usize];

		#[cfg(feature = "client")]
		{
			let _ = model.instances.swap_remove(instance.instance_id as usize);
		}

		// now, we need to know which instance owns that swapped one in O(1)
		model.handles.swap_remove(instance.instance_id as usize);

		// update the instance that was swapped
		let handle = model.handles[instance.instance_id as usize];
		if let Some(other) = self.instances.get_mut(*handle) {
			other.instance_id = instance.instance_id;
		}

		Some(instance)
	}

	#[cfg(not(feature = "server"))]
	pub fn add_instance(&mut self, model_id: u32, instance: InstanceBuilder) {
		let _ = self.packet_tx.send(Packet::CreateInstance {
			id: InstanceId::NONE,
			options: CreateInstance::from_builder(&instance, model_id),
		});
	}

	#[cfg(feature = "server")]
	pub fn add_instance(&mut self, model_id: u32, instance: InstanceBuilder) -> InstanceHandle {
		use std::sync::atomic::Ordering;

		let instance_id = InstanceId::new(self.next_instance_id.fetch_add(1, Ordering::SeqCst));

		let _ = self.packet_tx.send(Packet::CreateInstance {
			id: instance_id,
			options: CreateInstance::from_builder(&instance, model_id),
		});

		let handle = self.add_instance_local(model_id, instance);

		self.instance_ids.insert(handle, instance_id);
		self.handles.insert(instance_id, handle);

		handle
	}

	/// Adds a new instance to the drum and physics world.
	///
	/// If you only want to insert a rigidbody with no model,
	/// use [`Context::add_rigidbody`] or [`Context::add_collider`] instead.
	///
	/// The networked version of this method is [`Context::add_instance`].
	pub fn add_instance_local(
		&mut self,
		model_id: u32,
		mut instance: InstanceBuilder,
	) -> InstanceHandle {
		let model = &mut self.drum.models[model_id as usize];
		let instance_id = model.handles.len() as u32;

		let handle = self.instances.insert_with(|handle| {
			let handle = InstanceHandle::new(handle);
			let (axis, angle) = instance.rotation.to_axis_angle();

			instance.collider = instance
				.collider
				.or_else(|| Some(model.bounds.to_cuboid(instance.scale).mass(1.0)));

			instance.rigidbody = instance.rigidbody.map(|body| {
				body.position(instance.position.into())
					.rotation((axis * angle).into())
					.user_data(handle.into())
			});

			let instance = self.physics.add_instance(instance, model_id, instance_id);

			model.add_gpu_instance(&instance, handle, &self.physics);

			instance
		});

		InstanceHandle::new(handle)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstanceHandle {
	handle: Index,
}

pub struct InstancePair<'d> {
	#[cfg(feature = "client")]
	pub gpu: &'d GpuInstance,
	pub instance: &'d Instance,
}

pub struct InstancePairMut<'d> {
	pub instance: &'d mut Instance,

	#[cfg(feature = "client")]
	pub gpu: &'d mut GpuInstance,
	#[cfg(feature = "client")]
	dirty: &'d mut bool,
}

impl InstancePairMut<'_> {
	pub fn update<F>(&mut self, physics: &mut PhysicsState, update: F)
	where
		F: FnOnce(&mut Instance, &mut PhysicsState),
	{
		update(self.instance, physics);

		self.update_gpu(physics);
	}

	pub fn update_gpu(&mut self, physics: &PhysicsState) {
		#[cfg(feature = "client")]
		{
			*self.gpu = self.instance.to_gpu(physics);
			*self.dirty = true;
		}
	}
}

impl InstanceHandle {
	#[must_use]
	pub fn new(handle: Index) -> Self {
		Self { handle }
	}

	/// Returns the instance associated with the handle.
	///
	/// # Panics
	///
	/// Panics if the handle is invalid.
	#[must_use]
	pub fn resolve<'d>(
		&self,
		drum: &'d GpuDrum,
		instances: &'d Arena<Instance>,
	) -> Option<InstancePair<'d>> {
		let instance = instances.get(self.handle)?;
		#[cfg(feature = "client")]
		let model = drum.models.get(instance.model_id as usize)?;

		Some(InstancePair {
			#[cfg(feature = "client")]
			gpu: model.instances.get(instance.instance_id as usize)?,
			instance,
		})
	}

	/// Returns the instance associated with the handle.
	///
	/// # Panics
	///
	/// Panics if the handle is invalid.
	pub fn resolve_mut<'d>(
		&self,
		drum: &'d mut GpuDrum,
		instances: &'d mut Arena<Instance>,
	) -> Option<InstancePairMut<'d>> {
		let instance = instances.get_mut(self.handle)?;
		#[cfg(feature = "client")]
		let model = drum.models.get_mut(instance.model_id as usize)?;

		Some(InstancePairMut {
			#[cfg(feature = "client")]
			gpu: model.instances.get_mut(instance.instance_id as usize)?,
			#[cfg(feature = "client")]
			dirty: &mut model.dirty,
			instance,
		})
	}

	/// Returns the model associated with the instance.
	///
	/// # Panics
	///
	/// Panics if the handle is invalid.
	#[must_use]
	pub fn resolve_model<'d>(
		&self,
		drum: &'d GpuDrum,
		instances: &Arena<Instance>,
	) -> Option<&'d GpuModel> {
		let instance = instances.get(self.handle)?;

		drum.models.get(instance.model_id as usize)
	}

	/// Returns the model associated with the instance.
	///
	/// # Panics
	///
	/// Panics if the handle is invalid.
	pub fn resolve_model_mut<'d>(
		&self,
		drum: &'d mut GpuDrum,
		instances: &Arena<Instance>,
	) -> Option<&'d mut GpuModel> {
		let instance = instances.get(self.handle)?;

		drum.models.get_mut(instance.model_id as usize)
	}

	/// Updates the instance with the provided closure.
	///
	/// # Examples
	///
	/// ```rust,no_compile
	/// use ira::{physics::InstanceHandle, game::Context};
	///
	/// let ctx: &mut Context = ...;
	/// let instance: InstanceHandle = ...;
	///
	/// instance.update(ctx, |instance, physics| {
	///   instance.rotate_y(physics, 0.1);
	/// });
	/// ```
	pub fn update<F, M>(&self, ctx: &mut Context<M>, update: F)
	where
		F: FnOnce(&mut Instance, &mut PhysicsState),
	{
		let Some(mut pair) = self.resolve_mut(&mut ctx.drum, &mut ctx.instances) else {
			return;
		};

		pair.update(&mut ctx.physics, update);
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

impl ops::Deref for InstanceHandle {
	type Target = Index;

	fn deref(&self) -> &Self::Target {
		&self.handle
	}
}
