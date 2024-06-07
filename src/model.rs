use std::{fmt, mem, ops::Range, path::Path};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3};
use gltf::Gltf;
use wgpu::util::DeviceExt;

use crate::texture::{self, Material};

#[derive(Debug)]
pub struct Vertex {
	pub position: Vec3,
	pub normal: Vec3,
	pub tex_coords: Vec2,
	pub tangent: Vec3,
}

impl Vertex {
	pub const VERTICES: [wgpu::VertexAttribute; 4] =
		wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 3 => Float32x3];

	#[must_use]
	pub fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &Self::VERTICES,
		}
	}

	#[must_use]
	pub fn new(position: Vec3, normal: Vec3, tex_coords: Vec2) -> Self {
		Self {
			position,
			normal,
			tex_coords,
			tangent: Vec3::ZERO,
		}
	}

	#[must_use]
	pub fn into_gpu(self) -> GpuVertex {
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

#[derive(Debug, Default)]
pub struct Meshes {
	pub opaque: Vec<Mesh>,
	pub transparent: Vec<Mesh>,
}

#[must_use]
#[derive(Debug)]
pub struct Model {
	pub meshes: Meshes,
	pub materials: Vec<GpuMaterial>,
	pub centroid: Vec3,

	pub instances: Vec<Instance>,
}

pub struct GpuModel {
	pub model: Model,
	pub instance_buffer: wgpu::Buffer,
	pub instances: Vec<GpuInstance>,
}

#[must_use]
#[derive(Debug, Default)]
pub struct Instance {
	pub position: Vec3,
	pub rotation: glam::Quat,
}

#[must_use]
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct GpuInstance {
	pub model: [[f32; 4]; 4],
}

impl Instance {
	const VERTICES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
		4 => Float32x4,
		5 => Float32x4,
		6 => Float32x4,
		7 => Float32x4,
	];

	#[must_use]
	pub fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<GpuInstance>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Instance,
			attributes: &Self::VERTICES,
		}
	}

	pub fn new(position: Vec3, rotation: glam::Quat) -> Self {
		Self { position, rotation }
	}

	/// Creates an instance with the given position and up vector.
	///
	/// This is particularly useful when importing a model with a different up vector.
	/// The up vector used in Ira is [`Vec3::Y`].
	pub fn from_up(position: Vec3, up: Vec3) -> Self {
		Self {
			position,
			rotation: glam::Quat::from_rotation_arc(up, Vec3::Y),
		}
	}

	pub fn to_gpu(&self) -> GpuInstance {
		GpuInstance {
			model: (Mat4::from_translation(self.position) * Mat4::from_quat(self.rotation))
				.to_cols_array_2d(),
		}
	}
}

#[derive(Debug)]
pub struct Mesh {
	pub name: String,
	pub vertex_buffer: wgpu::Buffer,
	pub index_buffer: wgpu::Buffer,
	pub num_elements: u32,
	pub material: usize,
}

#[derive(Debug)]
pub struct GpuMaterial {
	material: Material,
	bind_group: wgpu::BindGroup,
}

impl Model {
	pub fn into_gpu(self, device: &wgpu::Device) -> GpuModel {
		let instances = self
			.instances
			.iter()
			.map(Instance::to_gpu)
			.collect::<Vec<_>>();

		let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
			label: Some("Instance Buffer"),
			contents: bytemuck::cast_slice(&instances),
			usage: wgpu::BufferUsages::VERTEX,
		});

		GpuModel {
			model: self,
			instance_buffer,
			instances,
		}
	}

	pub fn with_instance(mut self, instance: Instance) -> Self {
		self.instances.push(instance);
		self
	}

	/// Load a model from a glTF or OBJ file.
	///
	/// # Errors
	/// See [`Model::from_path_gltf`] and [`Model::from_path_obj`] for possible errors.
	pub async fn from_path<P>(
		path: P,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		layout: &wgpu::BindGroupLayout,
		cubemap: &texture::Texture,
	) -> anyhow::Result<Self>
	where
		P: AsRef<Path> + fmt::Debug,
	{
		match path.as_ref().extension().and_then(|s| s.to_str()) {
			Some("gltf") => Self::from_path_gltf(path, device, queue, layout, cubemap).await,
			_ => Err(anyhow::anyhow!("unsupported model format")),
		}
	}

	/// Loads a model from a glTF file.
	///
	/// # Errors
	/// Returns an error if the file cannot be read or the glTF data is invalid.
	/// Returns an error if the glTF file references external data and the root path cannot be determined.
	pub async fn from_path_gltf<P>(
		path: P,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		layout: &wgpu::BindGroupLayout,
		cubemap: &texture::Texture,
	) -> anyhow::Result<Self>
	where
		P: AsRef<Path> + fmt::Debug,
	{
		let root = path
			.as_ref()
			.parent()
			.ok_or_else(|| anyhow::anyhow!("expected path to file"))?;
		let gltf = Gltf::open(path.as_ref())?;

		let mut materials = Vec::new();

		for material in gltf.materials() {
			let material = texture::Material::from_gltf_material(device, queue, &material, root)?;
			let bind_group = material.create_bind_group(device, layout, cubemap);

			materials.push(GpuMaterial {
				material,
				bind_group,
			});
		}

		let buffers = gltf
			.buffers()
			.map(|b| {
				let data = gltf::buffer::Data::from_source(b.source(), Some(root))?;

				Ok(data.0)
			})
			.collect::<Result<Vec<_>, anyhow::Error>>()?;

		let mut centroid = Vec3::ZERO;
		let mut num_vertices = 0;

		let mut meshes = Meshes::default();

		for mesh in gltf.meshes() {
			for primitive in mesh.primitives() {
				let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

				let material_index = primitive.material().index();

				let positions = reader
					.read_positions()
					.ok_or_else(|| anyhow::anyhow!("mesh primitive does not have positions"))?;
				let normals = reader
					.read_normals()
					.ok_or_else(|| anyhow::anyhow!("mesh primitive does not have normals"))?;
				let tex_coords = reader.read_tex_coords(0);

				let vertices = positions.zip(normals);
				let mut vertices = if let Some(tex_coords) = tex_coords {
					vertices
						.zip(tex_coords.into_f32())
						.map(|((position, normal), tex_coord)| {
							centroid += Vec3::from(position);

							Vertex::new(
								position.into(),
								normal.into(),
								tex_coord.map(f32::fract).into(),
							)
						})
						.collect::<Vec<_>>()
				} else {
					vertices
						.map(|(position, normal)| {
							centroid += Vec3::from(position);

							Vertex::new(position.into(), normal.into(), Vec2::ZERO)
						})
						.collect::<Vec<_>>()
				};

				num_vertices += vertices.len();

				let indices = reader
					.read_indices()
					.ok_or_else(|| anyhow::anyhow!("mesh primitive does not have indices"))?
					.into_u32()
					.collect::<Vec<_>>();

				texture::compute_tangents(&mut vertices, &indices);

				let vertices = vertices
					.into_iter()
					.map(Vertex::into_gpu)
					.collect::<Vec<_>>();

				let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{path:?} Vertex Buffer")),
					contents: bytemuck::cast_slice(&vertices),
					usage: wgpu::BufferUsages::VERTEX,
				});
				let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{path:?} Index Buffer")),
					contents: bytemuck::cast_slice(&indices),
					usage: wgpu::BufferUsages::INDEX,
				});

				let mesh = Mesh {
					name: mesh.name().unwrap_or_default().to_string(),
					vertex_buffer,
					index_buffer,
					num_elements: indices.len() as u32,
					material: material_index.unwrap_or(0),
				};

				if materials[mesh.material].material.transparent {
					meshes.transparent.push(mesh);
				} else {
					meshes.opaque.push(mesh);
				}
			}
		}

		centroid /= num_vertices as f32;

		Ok(Model::new(meshes, materials, centroid))
	}

	pub fn new(meshes: Meshes, materials: Vec<GpuMaterial>, centroid: Vec3) -> Self {
		Self {
			meshes,
			materials,
			centroid,
			instances: Vec::new(),
		}
	}
}

pub trait DrawModel<'a> {
	fn draw_mesh_instanced(
		&mut self,
		mesh: &'a Mesh,
		material: &'a GpuMaterial,
		instances: Range<u32>,
		camera_bind_group: &'a wgpu::BindGroup,
		light_bind_group: &'a wgpu::BindGroup,
	);

	fn draw_model_instanced(
		&mut self,
		gpu_model: &'a GpuModel,
		camera_bind_group: &'a wgpu::BindGroup,
		light_bind_group: &'a wgpu::BindGroup,
		transparent: bool,
	);
}

impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
	'b: 'a,
{
	fn draw_mesh_instanced(
		&mut self,
		mesh: &'b Mesh,
		material: &'a GpuMaterial,
		instances: Range<u32>,
		camera_bind_group: &'b wgpu::BindGroup,
		light_bind_group: &'b wgpu::BindGroup,
	) {
		self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
		self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
		self.set_bind_group(0, &material.bind_group, &[]);
		self.set_bind_group(1, camera_bind_group, &[]);
		self.set_bind_group(2, light_bind_group, &[]);
		self.draw_indexed(0..mesh.num_elements, 0, instances);
	}

	fn draw_model_instanced(
		&mut self,
		gpu_model: &'b GpuModel,
		camera_bind_group: &'b wgpu::BindGroup,
		light_bind_group: &'b wgpu::BindGroup,
		transparent: bool,
	) {
		let model = &gpu_model.model;

		let meshes = if transparent {
			&model.meshes.transparent
		} else {
			&model.meshes.opaque
		};

		self.set_vertex_buffer(1, gpu_model.instance_buffer.slice(..));

		for mesh in meshes {
			let material = &model.materials[mesh.material];

			self.draw_mesh_instanced(
				mesh,
				material,
				0..model.instances.len() as u32,
				camera_bind_group,
				light_bind_group,
			);
		}
	}
}
