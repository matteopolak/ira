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

pub struct Model {
	pub meshes: Vec<Mesh>,
	pub materials: Vec<Material>,
}

pub struct Material {
	pub name: String,
	pub diffuse_texture: texture::Texture,
	pub bind_group: wgpu::BindGroup,
}

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
		let gltf = Gltf::open(path.as_ref())?;

		let mut materials = Vec::new();

		for material in gltf.materials() {
			let diffuse_texture = material
				.pbr_metallic_roughness()
				.base_color_texture()
				.map(|t| t.texture().source());
			let Some(diffuse_texture) = diffuse_texture else {
				continue;
			};

			let diffuse_texture =
				texture::Texture::from_image(device, queue, diffuse_texture, Some("mat texture"))?;

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
				diffuse_texture,
				bind_group,
			});
		}

		let buffers = gltf
			.buffers()
			.map(|b| {
				let data = gltf::buffer::Data::from_source(b.source(), None)?;

				Ok(data.0)
			})
			.collect::<Result<Vec<_>, anyhow::Error>>()?;

		let mut meshes = Vec::new();

		for mesh in gltf.meshes() {
			let mut vertices = Vec::new();
			let mut indices = Vec::new();

			for primitive in mesh.primitives() {
				let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

				let positions = reader.read_positions().unwrap();
				let normals = reader.read_normals().unwrap();
				let tex_coords = reader.read_tex_coords(0).unwrap();

				for ((position, normal), tex_coord) in
					positions.zip(normals).zip(tex_coords.into_f32())
				{
					vertices.push(Vertex {
						position,
						normal,
						tex_coords: tex_coord,
					});
				}

				indices.extend(reader.read_indices().unwrap().into_u32());
			}

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

			meshes.push(Mesh {
				name: mesh.name().unwrap_or_default().to_string(),
				vertex_buffer,
				index_buffer,
				num_elements: indices.len() as u32,
				material: mesh.index(),
			});
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

		for m in obj_materials? {
			let Some(diffuse_texture) = m.diffuse_texture else {
				continue;
			};

			let diffuse_texture =
				texture::Texture::from_path(device, queue, &diffuse_texture, "mat texture")?;

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
				name: m.name,
				diffuse_texture,
				bind_group,
			})
		}

		let meshes = models
			.into_iter()
			.map(|m| {
				let vertices = (0..m.mesh.positions.len() / 3)
					.map(|i| {
						if m.mesh.normals.is_empty() {
							Vertex {
								position: [
									m.mesh.positions[i * 3],
									m.mesh.positions[i * 3 + 1],
									m.mesh.positions[i * 3 + 2],
								],
								tex_coords: [
									m.mesh.texcoords[i * 2],
									1.0 - m.mesh.texcoords[i * 2 + 1],
								],
								normal: [0.0, 0.0, 0.0],
							}
						} else {
							Vertex {
								position: [
									m.mesh.positions[i * 3],
									m.mesh.positions[i * 3 + 1],
									m.mesh.positions[i * 3 + 2],
								],
								tex_coords: [
									m.mesh.texcoords[i * 2],
									1.0 - m.mesh.texcoords[i * 2 + 1],
								],
								normal: [
									m.mesh.normals[i * 3],
									m.mesh.normals[i * 3 + 1],
									m.mesh.normals[i * 3 + 2],
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
					contents: bytemuck::cast_slice(&m.mesh.indices),
					usage: wgpu::BufferUsages::INDEX,
				});

				Mesh {
					name: path.to_string(),
					vertex_buffer,
					index_buffer,
					num_elements: m.mesh.indices.len() as u32,
					material: m.mesh.material_id.unwrap_or(0),
				}
			})
			.collect::<Vec<_>>();

		Ok(Model { meshes, materials })
	}
}

pub trait DrawModel<'a> {
	fn draw_mesh(&mut self, mesh: &'a Mesh);
	fn draw_mesh_instanced(&mut self, mesh: &'a Mesh, instances: Range<u32>);
}
impl<'a, 'b> DrawModel<'b> for wgpu::RenderPass<'a>
where
	'b: 'a,
{
	fn draw_mesh(&mut self, mesh: &'b Mesh) {
		self.draw_mesh_instanced(mesh, 0..1);
	}

	fn draw_mesh_instanced(&mut self, mesh: &'b Mesh, instances: Range<u32>) {
		self.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
		self.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
		self.draw_indexed(0..mesh.num_elements, 0, instances);
	}
}
