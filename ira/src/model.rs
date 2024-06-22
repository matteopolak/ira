use glam::{Quat, Vec3};
use ira_drum::Handle;
use rapier3d::{
	dynamics::{RigidBody, RigidBodyBuilder, RigidBodyHandle},
	geometry::{ColliderBuilder, ColliderHandle},
};

use crate::{
	physics::{BoundingBox, InstanceHandle, PhysicsState},
	GpuDrum, GpuMesh,
};

#[cfg(feature = "client")]
use crate::render::model::GpuInstance;

#[derive(Debug)]
pub struct GpuModel {
	pub(crate) name: Box<str>,

	#[cfg(feature = "client")]
	pub(crate) meshes: GpuMeshHandles,
	#[cfg(feature = "client")]
	pub(crate) instance_buffer: wgpu::Buffer,

	// INVARIANT: instances.len() == handles.len()
	#[cfg(feature = "client")]
	pub(crate) instances: Vec<GpuInstance>,
	pub(crate) handles: Vec<InstanceHandle>,
	pub(crate) bounds: BoundingBox,

	#[cfg(feature = "client")]
	pub(crate) last_instance_count: usize,
	#[cfg(feature = "client")]
	pub(crate) dirty: bool,
}

impl GpuModel {
	#[must_use]
	pub fn new(
		drum: &GpuDrum,
		name: Box<str>,
		meshes: GpuMeshHandles,
		#[cfg(feature = "client")] device: &wgpu::Device,
	) -> Self {
		let (min, max) = meshes.min_max(drum);

		Self {
			handles: Vec::new(),
			bounds: BoundingBox { min, max },
			name,

			#[cfg(feature = "client")]
			meshes,
			#[cfg(feature = "client")]
			instance_buffer: Self::create_instance_buffer(device, &[]),
			#[cfg(feature = "client")]
			last_instance_count: 0,
			#[cfg(feature = "client")]
			instances: Vec::new(),
			#[cfg(feature = "client")]
			dirty: false,
		}
	}

	pub fn add_gpu_instance(&mut self, instance: &Instance, physics: &PhysicsState) {
		#[cfg(feature = "client")]
		self.instances.push(instance.to_gpu(physics));
	}

	#[must_use]
	pub fn bounds(&self) -> BoundingBox {
		self.bounds
	}
}

#[must_use]
#[derive(Debug)]
pub struct Instance {
	pub scale: Vec3,

	pub body: Body,
	pub collider: Option<ColliderHandle>,

	// used to index into the Vec<GpuInstance> in GpuModel
	pub(crate) instance_id: u32,
	// used to index into the Vec<GpuModel> in GpuDrum
	pub(crate) model_id: u32,
}

impl From<(RigidBodyHandle, ColliderHandle)> for Instance {
	fn from(value: (RigidBodyHandle, ColliderHandle)) -> Self {
		Self {
			scale: Vec3::ONE,
			body: Body::Rigid(value.0),
			collider: Some(value.1),
			instance_id: 0,
			model_id: 0,
		}
	}
}

impl Instance {
	/// Creates a new instance builder.
	///
	/// # Examples
	///
	/// ```rust
	/// let instance = Instance::builder()
	///   .position(Vec3::new(0.0, 10.0, 0.0))
	///   .scale(Vec3::splat(1.0))
	///   .rigidbody(RigidBodyBuilder::dynamic())
	///   .collider(ColliderBuilder::cuboid(1.0, 1.0, 1.0));
	///   .build();
	/// ```
	pub fn builder() -> InstanceBuilder {
		InstanceBuilder::default()
	}

	/// Returns the scale of the instance. By default, this is [`Vec3::ONE`].
	#[must_use]
	pub fn scale(&self) -> Vec3 {
		self.scale
	}

	/// Returns the body of the instance.
	#[must_use]
	pub fn body(&self) -> &Body {
		&self.body
	}

	/// Returns a handle to the collider attached to the rigidbody, if it exists.
	#[must_use]
	pub fn collider_handle(&self) -> Option<ColliderHandle> {
		self.collider
	}

	/// Rotates the instance around the Y axis. Note that this is equivalent to
	/// teleporting the instance to a new rotation.
	pub fn rotate_y(&mut self, physics: &mut PhysicsState, rad: f32) {
		self.body.rotate_y(physics, rad);
	}

	/// Moves and rotates the rigidbody (if it exists).
	pub fn set_position_rotation(
		&mut self,
		physics: &mut PhysicsState,
		position: Vec3,
		rotation: Quat,
	) {
		match &mut self.body {
			Body::Static {
				position: pos,
				rotation: rot,
			} => {
				*pos = position;
				*rot = rotation;
			}
			Body::Rigid(handle) => {
				let Some(body) = physics.rigid_bodies.get_mut(*handle) else {
					return;
				};

				if body.is_kinematic() {
					body.set_next_kinematic_position((position, rotation).into());
				} else {
					body.set_position((position, rotation).into(), true);
				}
			}
		}
	}
}

#[must_use]
#[derive(Debug, Clone)]
pub struct InstanceBuilder {
	pub position: Vec3,
	pub rotation: Quat,
	pub scale: Vec3,

	pub rigidbody: Option<RigidBodyBuilder>,
	pub collider: Option<ColliderBuilder>,
}

impl Default for InstanceBuilder {
	fn default() -> Self {
		Self {
			position: Vec3::ZERO,
			rotation: Quat::IDENTITY,
			scale: Vec3::ONE,
			rigidbody: None,
			collider: None,
		}
	}
}

impl InstanceBuilder {
	/// Sets the position of the instance.
	///
	/// Do not use this to update the position of the instance. Instead, use
	/// [`update_instance`](GpuModel::update_instance).
	pub fn position(mut self, position: Vec3) -> Self {
		self.position = position;
		self
	}

	/// Rotates the instance such that `up` rotates the instance to the up vector
	/// of the engine (which is `Vec3::Y`).
	///
	/// This is particularly useful when importing a model with a different up vector.
	/// The up vector used in Ira is [`Vec3::Y`].
	pub fn up(mut self, up: Vec3) -> Self {
		self.rotation = Quat::from_rotation_arc(up, Vec3::Y) * self.rotation;
		self
	}

	/// Rotates the instance by the given quaternion.
	///
	/// Once built, this rotation will be applied to the instance's rigidbody,
	/// or collider if the rigidbody doesn't exist.
	pub fn rotation(mut self, rotation: Quat) -> Self {
		self.rotation = rotation * self.rotation;
		self
	}

	/// Sets the scale of the instance. By default, this is [`Vec3::ONE`].
	///
	/// This scale is applied to the rendered model and the default collider.
	/// If you create your own collider, this scale will not be applied to it.
	pub fn scale(mut self, scale: Vec3) -> Self {
		self.scale = scale;
		self
	}

	/// Sets the rigidbody of the instance.
	///
	/// Note that you should provide a collider to the rigidbody if you want
	/// to set the mass or use a non-cuboid collider. See [`Self::collider`] and
	/// [`ColliderBuilder::mass`].
	pub fn rigidbody(mut self, rigidbody: RigidBodyBuilder) -> Self {
		self.rigidbody = Some(rigidbody);
		self
	}

	/// Sets the collider of the instance.
	///
	/// Note that you should provide mass to the collider if you want the instance to be
	/// affected by physics. See [`ColliderBuilder::mass`].
	///
	/// By default, a collider is created from the model's bounds with a mass of `1.0`
	/// if a rigidbody is provided but no collider is provided.
	pub fn collider(mut self, collider: ColliderBuilder) -> Self {
		self.collider = Some(collider);
		self
	}
}

/// The body of an instance.
///
/// When using [`Body::Static`], the instance is not affected by any physics.
/// If you want a static instance to be affected by physics, use a kinematic rigidbody.
///
/// When using [`Body::Rigid`], the instance is affected according to the rules of
/// the rigidbody. See [`RigidBodyBuilder`] for more information.
#[derive(Debug)]
pub enum Body {
	Static { position: Vec3, rotation: Quat },
	Rigid(RigidBodyHandle),
}

impl Body {
	pub fn rotate_y(&mut self, physics: &mut PhysicsState, rad: f32) {
		match self {
			Self::Static { rotation, .. } => *rotation = Quat::from_rotation_y(rad) * *rotation,
			Self::Rigid(handle) => {
				let Some(body) = physics.rigid_bodies.get_mut(*handle) else {
					return;
				};
				let current_rotation: Quat = body.position().rotation.into();
				let new_rotation = Quat::from_rotation_y(rad) * current_rotation;

				if body.is_kinematic() {
					body.set_next_kinematic_rotation(new_rotation.into());
				} else {
					body.set_rotation(new_rotation.into(), true);
				}
			}
		}
	}

	pub fn update<F>(&mut self, physics: &mut PhysicsState, update: F)
	where
		F: FnOnce(&mut RigidBody),
	{
		match self {
			Self::Static { .. } => {}
			Self::Rigid(handle) => {
				let Some(body) = physics.rigid_bodies.get_mut(*handle) else {
					return;
				};

				update(body);
			}
		}
	}

	pub fn set_transform(&mut self, physics: &mut PhysicsState, position: Vec3, rotation: Quat) {
		match self {
			Self::Static {
				position: p,
				rotation: r,
			} => {
				*p = position;
				*r = rotation;
			}
			Self::Rigid(handle) => {
				let Some(body) = physics.rigid_bodies.get_mut(*handle) else {
					return;
				};

				if body.is_kinematic() {
					body.set_next_kinematic_position((position, rotation).into());
				} else {
					body.set_position((position, rotation).into(), true);
				}
			}
		}
	}

	#[must_use]
	pub fn pos_rot(&self, physics: &PhysicsState) -> (Vec3, Quat) {
		match self {
			Self::Static { position, rotation } => (*position, *rotation),
			Self::Rigid(handle) => {
				let position = physics
					.rigid_bodies
					.get(*handle)
					.map_or_else(Default::default, |r| *r.position());

				(position.translation.vector.into(), position.rotation.into())
			}
		}
	}
}

pub trait ModelExt {
	/// Converts the model into a GPU model.
	fn into_gpu(self, drum: &GpuDrum, #[cfg(feature = "client")] device: &wgpu::Device)
		-> GpuModel;
}

impl ModelExt for ira_drum::Model {
	fn into_gpu(
		self,
		drum: &GpuDrum,
		#[cfg(feature = "client")] device: &wgpu::Device,
	) -> GpuModel {
		GpuModel::new(
			drum,
			self.name,
			self.meshes.into(),
			#[cfg(feature = "client")]
			device,
		)
	}
}

#[derive(Debug)]
pub struct GpuMeshHandles {
	pub opaque: Box<[Handle<GpuMesh>]>,
	pub transparent: Box<[Handle<GpuMesh>]>,
}

impl GpuMeshHandles {
	#[must_use]
	pub fn min_max(&self, drum: &GpuDrum) -> (Vec3, Vec3) {
		let mut min = Vec3::splat(f32::INFINITY);
		let mut max = Vec3::splat(f32::NEG_INFINITY);

		for mesh in self.opaque.iter().chain(self.transparent.iter()) {
			let mesh = mesh.resolve(&drum.meshes);

			min = min.min(mesh.min);
			max = max.max(mesh.max);
		}

		(min, max)
	}
}

impl From<ira_drum::MeshHandles> for GpuMeshHandles {
	fn from(meshes: ira_drum::MeshHandles) -> Self {
		Self {
			opaque: meshes.opaque.iter().map(|h| Handle::new(h.raw())).collect(),
			transparent: meshes
				.transparent
				.iter()
				.map(|h| Handle::new(h.raw()))
				.collect(),
		}
	}
}
