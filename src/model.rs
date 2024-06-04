use std::{
	fmt,
	fs::File,
	io::BufReader,
	mem,
	ops::Range,
	path::{Path, PathBuf},
};

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use gltf::Gltf;
use wgpu::util::DeviceExt;

use crate::texture::{self, Material};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
	pub position: [f32; 3],
	pub normal: [f32; 3],
	pub tex_coords: [f32; 2],
}

impl Vertex {
	pub const VERTICES: [wgpu::VertexAttribute; 3] =
		wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2];

	#[must_use]
	pub fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &Self::VERTICES,
		}
	}

	#[must_use]
	pub fn new(position: [f32; 3], normal: [f32; 3], tex_coords: [f32; 2]) -> Self {
		Self {
			position,
			normal,
			tex_coords,
		}
	}
}

#[derive(Debug, Default)]
pub struct Meshes {
	pub opaque: Vec<Mesh>,
	pub transparent: Vec<Mesh>,
}

#[derive(Debug)]
pub struct Model {
	pub meshes: Meshes,
	pub materials: Vec<GpuMaterial>,
	pub centroid: Vec3,
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
	/// Load a model from a glTF or OBJ file.
	///
	/// # Errors
	/// See [`Model::from_path_gltf`] and [`Model::from_path_obj`] for possible errors.
	pub async fn from_path<P>(
		path: P,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		layout: &wgpu::BindGroupLayout,
		up: Vec3,
	) -> anyhow::Result<Self>
	where
		P: AsRef<Path> + fmt::Debug,
	{
		match path.as_ref().extension().and_then(|s| s.to_str()) {
			Some("obj") => Self::from_path_obj(path, device, queue, layout).await,
			Some("gltf") => Self::from_path_gltf(path, device, queue, layout, up).await,
			_ => Err(anyhow::anyhow!("Unsupported model format")),
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
		up: Vec3,
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
			let bind_group = material.create_bind_group(device, layout);

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
				let mut vertices = Vec::new();

				let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

				let material_index = primitive.material().index();

				let positions = reader
					.read_positions()
					.ok_or_else(|| anyhow::anyhow!("mesh primitive does not have positions"))?;
				let normals = reader
					.read_normals()
					.ok_or_else(|| anyhow::anyhow!("mesh primitive does not have normals"))?;
				let tex_coords = reader.read_tex_coords(0);

				// rotate them to that `up` is now Vec3::Y
				let rotation = glam::Quat::from_rotation_arc(up, Vec3::Y);
				let positions = positions.map(|p| rotation.mul_vec3(p.into()));
				let normals = normals.map(|n| rotation.mul_vec3(n.into()));

				if let Some(tex_coords) = tex_coords {
					for ((position, normal), tex_coord) in
						positions.zip(normals).zip(tex_coords.into_f32())
					{
						vertices.push(Vertex::new(
							position.into(),
							normal.into(),
							tex_coord.map(f32::fract),
						));

						centroid += position;
					}
				} else {
					for (position, normal) in positions.zip(normals) {
						vertices.push(Vertex::new(position.into(), normal.into(), [0.0, 0.0]));

						centroid += position;
					}
				}

				num_vertices += vertices.len();

				let indices = reader
					.read_indices()
					.ok_or_else(|| anyhow::anyhow!("mesh primitive does not have indices"))?
					.into_u32()
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

		Ok(Model {
			meshes,
			materials,
			centroid,
		})
	}

	/// Loads a model from an OBJ file.
	///
	/// # Errors
	/// Returns an error if the file cannot be read or the OBJ data is invalid.
	/// Returns an error if the OBJ file references external data and the root path cannot be determined.
	pub async fn from_path_obj<P>(
		path: P,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		layout: &wgpu::BindGroupLayout,
	) -> anyhow::Result<Self>
	where
		P: AsRef<Path> + fmt::Debug,
	{
		let root = path
			.as_ref()
			.parent()
			.ok_or_else(|| anyhow::anyhow!("expected path to file"))?;
		let mut file = BufReader::new(File::open(&path)?);
		let (models, obj_materials) = tobj::load_obj_buf_async(
			&mut file,
			&tobj::LoadOptions {
				triangulate: true,
				single_index: true,
				..Default::default()
			},
			|p| async move {
				let path = root.join(PathBuf::from(p));
				let mut reader =
					BufReader::new(File::open(&path).map_err(|_| tobj::LoadError::OpenFileFailed)?);

				tobj::load_mtl_buf(&mut reader)
			},
		)
		.await?;

		let mut materials = Vec::new();

		for material in obj_materials? {
			let material = texture::Material::from_obj_material(device, queue, &material, root)?;
			let bind_group = material.create_bind_group(device, layout);

			materials.push(GpuMaterial {
				material,
				bind_group,
			});
		}

		let mut centroid = Vec3::ZERO;
		let mut num_vertices = 0;

		let mut meshes = Meshes::default();

		for model in models {
			let mesh = &model.mesh;
			let vertices = (0..mesh.positions.len() / 3)
				.map(|i| {
					let position = [
						mesh.positions[i * 3],
						mesh.positions[i * 3 + 1],
						mesh.positions[i * 3 + 2],
					];

					centroid += Vec3::from(position);

					/*if mesh.normals.is_empty() {
						Vertex {
							position,
							tex_coords: [mesh.texcoords[i * 2], 1.0 - mesh.texcoords[i * 2 + 1]],
							normal: [0.0, 0.0, 0.0],
						}
					} else {
						Vertex {
							position,
							tex_coords: [mesh.texcoords[i * 2], 1.0 - mesh.texcoords[i * 2 + 1]],
							normal: [
								mesh.normals[i * 3],
								mesh.normals[i * 3 + 1],
								mesh.normals[i * 3 + 2],
							],
						}
					}*/
				})
				.collect::<Vec<_>>();

			num_vertices += vertices.len();

			let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{path:?} Vertex Buffer")),
				contents: bytemuck::cast_slice(&vertices),
				usage: wgpu::BufferUsages::VERTEX,
			});
			let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{path:?} Index Buffer")),
				contents: bytemuck::cast_slice(&mesh.indices),
				usage: wgpu::BufferUsages::INDEX,
			});

			let mesh = Mesh {
				name: model.name,
				vertex_buffer,
				index_buffer,
				num_elements: mesh.indices.len() as u32,
				material: mesh.material_id.unwrap_or(0),
			};

			if materials[mesh.material].material.transparent {
				meshes.transparent.push(mesh);
			} else {
				meshes.opaque.push(mesh);
			}
		}

		centroid /= num_vertices as f32;

		Ok(Model {
			meshes,
			materials,
			centroid,
		})
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
		model: &'a Model,
		instances: Range<u32>,
		camera_bind_group: &'a wgpu::BindGroup,
		light_bind_group: &'a wgpu::BindGroup,
		transparent: bool,
	) {
		let meshes = if transparent {
			&model.meshes.transparent
		} else {
			&model.meshes.opaque
		};

		for mesh in meshes {
			let material = &model.materials[mesh.material];

			self.draw_mesh_instanced(
				mesh,
				material,
				instances.clone(),
				camera_bind_group,
				light_bind_group,
			);
		}
	}
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
}
