use std::mem;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Quat, Vec3};
use ira_drum::Handle;
use rapier3d::{
	dynamics::{RigidBody, RigidBodyBuilder, RigidBodyHandle},
	geometry::{ColliderBuilder, ColliderHandle},
};
use wgpu::util::DeviceExt;

use crate::{
	physics::{BoundingBox, InstanceHandle, PhysicsState},
	GpuDrum, GpuMesh,
};

pub trait VertexExt {
	const ATTRIBUTES: [wgpu::VertexAttribute; 4];

	fn desc() -> wgpu::VertexBufferLayout<'static>;
	fn into_gpu(self) -> GpuVertex;
}

impl VertexExt for ira_drum::Vertex {
	const ATTRIBUTES: [wgpu::VertexAttribute; 4] =
		wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x3];

	#[must_use]
	fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Self>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &Self::ATTRIBUTES,
		}
	}

	#[must_use]
	fn into_gpu(self) -> GpuVertex {
		GpuVertex {
			position: self.position.into(),
			normal: self.normal.into(),
			tex_coords: self.tex_coords.into(),
			tangent: self.tangent.into(),
		}
	}
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuVertex {
	pub position: [f32; 3],
	pub normal: [f32; 3],
	pub tex_coords: [f32; 2],
	pub tangent: [f32; 3],
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

#[derive(Debug)]
pub struct GpuModel {
	pub(crate) name: Box<str>,

	pub(crate) meshes: GpuMeshHandles,
	pub(crate) instance_buffer: wgpu::Buffer,

	// INVARIANT: instances.len() == handles.len()
	pub(crate) instances: Vec<GpuInstance>,
	pub(crate) handles: Vec<InstanceHandle>,
	pub(crate) bounds: BoundingBox,

	last_instance_count: usize,
	pub(crate) dirty: bool,
}

impl GpuModel {
	#[must_use]
	pub fn new(
		device: &wgpu::Device,
		drum: &GpuDrum,
		meshes: GpuMeshHandles,
		name: Box<str>,
	) -> Self {
		let (min, max) = meshes.min_max(drum);

		Self {
			name,

			meshes,
			instance_buffer: Self::create_instance_buffer(device, &[]),
			last_instance_count: 0,

			handles: Vec::new(),
			instances: Vec::new(),

			bounds: BoundingBox { min, max },
			dirty: false,
		}
	}

	pub fn bounds(&self) -> BoundingBox {
		self.bounds
	}

	#[must_use]
	pub fn create_instance_buffer(
		device: &wgpu::Device,
		instances: &[GpuInstance],
	) -> wgpu::Buffer {
		device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("instance buffer"),
			contents: bytemuck::cast_slice(instances),
			usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
		})
	}

	/// Recreates the instance buffer with the current instance data.
	pub fn recreate_instance_buffer(&mut self, device: &wgpu::Device) {
		self.instance_buffer = Self::create_instance_buffer(device, &self.instances);
	}

	/// Updates the instance buffer with the current instance data.
	///
	/// If the number of instances has changed, the buffer is recreated.
	pub fn update_instance_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
		if self.instances.len() != self.last_instance_count {
			self.recreate_instance_buffer(device);
			self.last_instance_count = self.instances.len();
		} else if self.dirty {
			queue.write_buffer(
				&self.instance_buffer,
				0,
				bytemuck::cast_slice(&self.instances),
			);
		}
	}

	pub(crate) fn draw_instanced<'r, 's: 'r>(
		&'s self,
		pass: &mut wgpu::RenderPass<'r>,
		drum: &'s GpuDrum,
		camera_bind_group: &'s wgpu::BindGroup,
		light_bind_group: &'s wgpu::BindGroup,
		brdf_bind_group: &'s wgpu::BindGroup,
		transparent: bool,
	) {
		let meshes = if transparent {
			&self.meshes.transparent
		} else {
			&self.meshes.opaque
		};

		pass.set_vertex_buffer(1, self.instance_buffer.slice(..));

		for mesh in meshes {
			let mesh = mesh.resolve(&drum.meshes);
			let material = mesh.material.resolve(&drum.materials);

			pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
			pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
			pass.set_bind_group(0, &material.bind_group, &[]);
			pass.set_bind_group(1, camera_bind_group, &[]);
			pass.set_bind_group(2, light_bind_group, &[]);
			pass.set_bind_group(3, brdf_bind_group, &[]);
			pass.draw_indexed(0..mesh.num_indices, 0, 0..self.instances.len() as _);
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
	pub(crate) const VERTICES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
		4 => Float32x4,
		5 => Float32x4,
		6 => Float32x4,
		7 => Float32x4,
	];

	pub(crate) fn new(model_id: u32, instance_id: u32) -> Self {
		Self {
			scale: Vec3::ONE,
			body: Body::Static {
				position: Vec3::ZERO,
				rotation: Quat::IDENTITY,
			},
			collider: None,
			instance_id,
			model_id,
		}
	}

	#[must_use]
	pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<GpuInstance>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &Self::VERTICES,
		}
	}

	pub(crate) fn to_gpu(&self, physics: &PhysicsState) -> GpuInstance {
		let (position, rotation) = match self.body {
			Body::Rigid(handle) => {
				let position = physics
					.rigid_bodies
					.get(handle)
					.map_or_else(Default::default, |r| *r.position());

				position.into()
			}
			Body::Static { position, rotation } => (position, rotation),
		};

		GpuInstance {
			model: Mat4::from_scale_rotation_translation(self.scale, rotation, position)
				.to_cols_array_2d(),
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
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuInstance {
	model: [[f32; 4]; 4],
}

impl GpuInstance {
	#[must_use]
	pub fn model(&self) -> Mat4 {
		Mat4::from_cols_array_2d(&self.model)
	}
}

pub trait ModelExt {
	/// Converts the model into a GPU model.
	fn into_gpu(self, device: &wgpu::Device, drum: &GpuDrum) -> GpuModel;
}

impl ModelExt for ira_drum::Model {
	fn into_gpu(self, device: &wgpu::Device, drum: &GpuDrum) -> GpuModel {
		GpuModel::new(device, drum, self.meshes.into(), self.name)
	}
}
