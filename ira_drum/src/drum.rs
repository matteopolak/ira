use bincode::{Decode, Encode};

#[derive(Debug, Encode, Decode)]
pub struct Drum {
	pub textures: Box<[super::Texture]>,
	pub materials: Box<[super::Material]>,
	pub meshes: Box<[super::Mesh]>,
	pub models: Box<[super::Model]>,
}

#[cfg(feature = "gltf")]
impl Drum {
	pub fn from_gltf(gltf: gltf::Gltf) {
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
	}
}
