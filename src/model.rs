use std::{
	fmt,
	fs::File,
	io::BufReader,
	mem,
	ops::Range,
	path::{Path, PathBuf},
};

use bytemuck::{Pod, Zeroable};
use gltf::Gltf;
use wgpu::util::DeviceExt;

use crate::texture;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
	pub position: [f32; 3],
	pub normal: [f32; 3],
	pub tex_coords: [f32; 2],
}

impl Vertex {
	const VERTICES: [wgpu::VertexAttribute; 3] =
		wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3];

	pub fn desc() -> wgpu::VertexBufferLayout<'static> {
		wgpu::VertexBufferLayout {
			array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
			step_mode: wgpu::VertexStepMode::Vertex,
			attributes: &Self::VERTICES,
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
	pub materials: Vec<Material>,
}

#[derive(Debug)]
pub struct Material {
	pub name: String,
	pub texture: texture::Texture,
	pub bind_group: wgpu::BindGroup,
}

#[derive(Debug)]
pub struct Mesh {
	pub name: String,
	pub vertex_buffer: wgpu::Buffer,
	pub index_buffer: wgpu::Buffer,
	pub num_elements: u32,
	pub material: usize,
}

impl Model {
	pub async fn from_path<P>(
		path: P,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		layout: &wgpu::BindGroupLayout,
	) -> anyhow::Result<Self>
	where
		P: AsRef<Path> + fmt::Display + fmt::Debug,
	{
		match path.as_ref().extension().and_then(|s| s.to_str()) {
			Some("obj") => Self::from_path_obj(path, device, queue, layout).await,
			Some("gltf") => Self::from_path_gltf(path, device, queue, layout).await,
			_ => Err(anyhow::anyhow!("Unsupported model format")),
		}
	}

	async fn from_path_gltf<P>(
		path: P,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		layout: &wgpu::BindGroupLayout,
	) -> anyhow::Result<Self>
	where
		P: AsRef<Path> + fmt::Display + fmt::Debug,
	{
		let root = path
			.as_ref()
			.parent()
			.ok_or_else(|| anyhow::anyhow!("expected path to file"))?;
		let gltf = Gltf::open(path.as_ref())?;

		let mut materials = Vec::new();

		for material in gltf.materials() {
			let diffuse_texture =
				texture::Texture::from_gltf_material(device, queue, &material, root)?;

			let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
				layout,
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
					},
				],
				label: None,
			});

			materials.push(Material {
				name: material.name().unwrap_or_default().to_string(),
				texture: diffuse_texture,
				bind_group,
			});
		}

		let buffers = gltf
			.buffers()
			.map(|b| {
				let data = gltf::buffer::Data::from_source(b.source(), Some(root))?;

				Ok(data.0)
			})
			.collect::<Result<Vec<_>, anyhow::Error>>()
			.unwrap();

		let mut meshes = Meshes::default();

		for mesh in gltf.meshes() {
			for primitive in mesh.primitives() {
				let mut vertices = Vec::new();

				let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

				let positions = reader.read_positions().unwrap();
				let normals = reader.read_normals().unwrap();
				let tex_coords = reader.read_tex_coords(0);

				let material_index = primitive.material().index();

				if let Some(tex_coords) = tex_coords {
					for ((position, normal), tex_coord) in
						positions.zip(normals).zip(tex_coords.into_f32())
					{
						vertices.push(Vertex {
							position,
							normal,
							tex_coords: tex_coord,
						});
					}
				} else {
					for (position, normal) in positions.zip(normals) {
						vertices.push(Vertex {
							position,
							normal,
							tex_coords: [0.0, 0.0],
						});
					}
				}

				let indices = reader
					.read_indices()
					.unwrap()
					.into_u32()
					.collect::<Vec<_>>();

				let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{:?} Vertex Buffer", path)),
					contents: bytemuck::cast_slice(&vertices),
					usage: wgpu::BufferUsages::VERTEX,
				});
				let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
					label: Some(&format!("{:?} Index Buffer", path)),
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

				if materials[mesh.material].texture.transparent {
					meshes.transparent.push(mesh);
				} else {
					meshes.opaque.push(mesh);
				}
			}
		}

		Ok(Model { meshes, materials })
	}

	async fn from_path_obj<P>(
		path: P,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		layout: &wgpu::BindGroupLayout,
	) -> anyhow::Result<Self>
	where
		P: AsRef<Path> + fmt::Display + fmt::Debug,
	{
		let root = path.as_ref().parent().unwrap();
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
				let mut reader = BufReader::new(File::open(&path).unwrap());

				tobj::load_mtl_buf(&mut reader)
			},
		)
		.await?;

		let mut materials = Vec::new();

		for material in obj_materials? {
			let diffuse_texture =
				texture::Texture::from_obj_material(device, queue, &material, root)?;

			let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
				layout,
				entries: &[
					wgpu::BindGroupEntry {
						binding: 0,
						resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
					},
					wgpu::BindGroupEntry {
						binding: 1,
						resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
					},
				],
				label: None,
			});

			materials.push(Material {
				name: material.name,
				texture: diffuse_texture,
				bind_group,
			});
		}

		let mut meshes = Meshes::default();

		for model in models {
			let mesh = &model.mesh;
			let vertices = (0..mesh.positions.len() / 3)
				.map(|i| {
					if mesh.normals.is_empty() {
						Vertex {
							position: [
								mesh.positions[i * 3],
								mesh.positions[i * 3 + 1],
								mesh.positions[i * 3 + 2],
							],
							tex_coords: [mesh.texcoords[i * 2], 1.0 - mesh.texcoords[i * 2 + 1]],
							normal: [0.0, 0.0, 0.0],
						}
					} else {
						Vertex {
							position: [
								mesh.positions[i * 3],
								mesh.positions[i * 3 + 1],
								mesh.positions[i * 3 + 2],
							],
							tex_coords: [mesh.texcoords[i * 2], 1.0 - mesh.texcoords[i * 2 + 1]],
							normal: [
								mesh.normals[i * 3],
								mesh.normals[i * 3 + 1],
								mesh.normals[i * 3 + 2],
							],
						}
					}
				})
				.collect::<Vec<_>>();

			let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{:?} Vertex Buffer", path)),
				contents: bytemuck::cast_slice(&vertices),
				usage: wgpu::BufferUsages::VERTEX,
			});
			let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
				label: Some(&format!("{:?} Index Buffer", path)),
				contents: bytemuck::cast_slice(&mesh.indices),
				usage: wgpu::BufferUsages::INDEX,
			});

			let mesh = Mesh {
				name: path.to_string(),
				vertex_buffer,
				index_buffer,
				num_elements: mesh.indices.len() as u32,
				material: mesh.material_id.unwrap_or(0),
			};

			if materials[mesh.material].texture.transparent {
				meshes.transparent.push(mesh);
			} else {
				meshes.opaque.push(mesh);
			}
		}

		Ok(Model { meshes, materials })
	}
}

pub trait DrawModel<'a> {
	fn draw_mesh_instanced(
		&mut self,
		mesh: &'a Mesh,
		material: &'a Material,
		instances: Range<u32>,
		camera_bind_group: &'a wgpu::BindGroup,
	);

	fn draw_model_instanced(
		&mut self,
		model: &'a Model,
		instances: Range<u32>,
		camera_bind_group: &'a wgpu::BindGroup,
		transparent: bool,
	) {
		let meshes = if transparent {
			&model.meshes.transparent
		} else {
			&model.meshes.opaque
		};

		for mesh in meshes {
			let material = &model.materials[mesh.material];

			self.draw_mesh_instanced(mesh, material, instances.clone(), camera_bind_group);
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
		material: &'a Material,
		instances: Range<u32>,
		camera_bind_group: &'b wgpu::BindGroup,
	) {
		self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
		self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
		self.set_bind_group(0, &material.bind_group, &[]);
		self.set_bind_group(1, camera_bind_group, &[]);
		self.draw_indexed(0..mesh.num_elements, 0, instances);
	}
}
