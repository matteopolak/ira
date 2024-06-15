use std::{
	collections::HashMap,
	fmt,
	fs::File,
	io::BufReader,
	mem,
	path::{Path, PathBuf},
};

use crate::{
	DrumBuilder, Handle, Material, Mesh, MeshHandles, Model, Source, Texture, Vec2, Vec3, Vertex,
};

#[must_use]
#[derive(Debug)]
pub struct ObjSource {
	models: Vec<tobj::Model>,
	materials: Vec<tobj::Material>,

	root: PathBuf,

	/// Textures already loaded. Used for deduplication.
	textures: HashMap<PathBuf, Handle<Texture>>,
}

impl ObjSource {
	/// Creates a new OBJ source.
	pub fn new(models: Vec<tobj::Model>, materials: Vec<tobj::Material>, root: PathBuf) -> Self {
		Self {
			models,
			materials,
			root,
			textures: HashMap::new(),
		}
	}

	/// Loads an OBJ file from the given path.
	///
	/// # Errors
	///
	/// See [`tobj::load_obj`].
	///
	/// # Panics
	///
	/// Panics if the given path does not have a parent directory.
	pub fn from_path<P: AsRef<Path> + fmt::Debug>(path: P) -> Result<Self, tobj::LoadError> {
		let mut reader = BufReader::new(File::open(&path).map_err(|_| tobj::LoadError::ReadError)?);
		let root = path.as_ref().parent().unwrap().to_path_buf();

		let (models, materials) =
			tobj::load_obj_buf(&mut reader, &tobj::OFFLINE_RENDERING_LOAD_OPTIONS, |p| {
				tobj::load_mtl(root.join(p))
			})?;

		Ok(Self::new(models, materials?, root))
	}

	/// Gets a texture from the given path.
	/// If the texture has already been loaded, the existing handle is returned.
	///
	/// # Errors
	///
	/// See [`Texture::from_path`].
	///
	/// # Panics
	///
	/// Panics if the given path could not be canonicalized.
	/// See [`Path::canonicalize`].
	pub fn get_texture<P: AsRef<Path>>(
		&mut self,
		drum: &mut DrumBuilder,
		path: P,
	) -> Result<Handle<Texture>, image::ImageError> {
		let path = self.root.join(path).canonicalize().unwrap();

		if let Some(handle) = self.textures.get(&path) {
			return Ok(*handle);
		}

		let texture = Texture::from_path(&path)?;
		let handle = drum.add_texture(texture);

		self.textures.insert(path, handle);

		Ok(handle)
	}
}

impl Source for ObjSource {
	type Error = tobj::LoadError;

	fn add_to_drum(mut self, drum: &mut DrumBuilder) -> Result<(), Self::Error> {
		let mut opaque_meshes = Vec::new();
		let mut transparent_meshes = Vec::new();

		let materials = mem::take(&mut self.materials);
		let material_handles = materials
			.iter()
			.map(|material| {
				let material = Material::from_obj_material(material, drum, &mut self);
				drum.add_material(material)
			})
			.collect::<Vec<_>>();

		self.materials = materials;

		let mut centroid = Vec3::ZERO;
		let mut num_vertices = 0;

		for model in self.models {
			let mesh = model.mesh;

			let handle = material_handles[mesh.material_id.unwrap_or(0)];

			let mut vertices = Vec::new();

			for i in 0..mesh.positions.len() / 3 {
				let position = [
					mesh.positions[i * 3],
					mesh.positions[i * 3 + 1],
					mesh.positions[i * 3 + 2],
				]
				.into();

				centroid += position;

				let normal = if mesh.normals.len() == mesh.positions.len() {
					[
						mesh.normals[i * 3],
						mesh.normals[i * 3 + 1],
						mesh.normals[i * 3 + 2],
					]
					.into()
				} else {
					Vec3::ZERO
				};

				let texcoord = if mesh.texcoords.len() == mesh.positions.len() {
					[mesh.texcoords[i * 2], mesh.texcoords[i * 2 + 1]].into()
				} else {
					Vec2::ZERO
				};

				vertices.push(Vertex::new(position, normal, texcoord));
			}

			num_vertices += vertices.len();

			let mesh = Mesh::new(
				vertices.into_boxed_slice(),
				mesh.indices.into_boxed_slice(),
				handle,
			);

			let material = handle.resolve(&drum.materials);

			if material.transparent {
				transparent_meshes.push(drum.add_mesh(mesh));
			} else {
				opaque_meshes.push(drum.add_mesh(mesh));
			}
		}

		centroid /= num_vertices as f32;

		let model = Model {
			meshes: MeshHandles {
				opaque: opaque_meshes.into_boxed_slice(),
				transparent: transparent_meshes.into_boxed_slice(),
			},
			center: centroid,
		};

		drum.add_model(model);

		Ok(())
	}
}

impl Material {
	/// Creates a new material from an OBJ material.
	///
	/// # Panics
	///
	/// Panics if the albedo texture could not be created.
	pub fn from_obj_material(
		material: &tobj::Material,
		drum: &mut DrumBuilder,
		source: &mut ObjSource,
	) -> Self {
		// TODO: use dissolve_texture
		let albedo = material
			.diffuse_texture
			.as_ref()
			.map(|file| source.get_texture(drum, file))
			.or_else(|| {
				let a = material.dissolve.unwrap_or(1.0);

				Ok(material.diffuse.map(|pixel| {
					drum.add_texture(Texture::from_rgba_pixel_f32([
						pixel[0], pixel[1], pixel[2], a,
					]))
				}))
				.transpose()
			})
			.unwrap_or_else(|| Ok(drum.add_texture(Texture::from_r_pixel(0))))
			.expect("failed to create albedo texture");

		let normal = material
			.normal_texture
			.as_ref()
			.map(|file| source.get_texture(drum, file))
			.transpose()
			.expect("failed to create normal texture");

		// ambient, 1-shininess/1000, shininess
		// TODO: handle ambient occlusion
		let orm = drum.add_texture(Texture::from_rgb_pixel_f32([
			0.0,
			1.0 - material.shininess.unwrap_or(0.0) / 1000.0,
			material.shininess.unwrap_or(0.0),
		]));

		Material {
			albedo,
			normal,
			orm: Some(orm),
			emissive: None,
			transparent: material.dissolve.unwrap_or(1.0) < 1.0,
		}
	}
}
