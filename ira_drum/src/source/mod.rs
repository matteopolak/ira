#[cfg(feature = "gltf")]
mod gltf;

#[cfg(feature = "obj")]
mod obj;

use crate::DrumBuilder;

pub trait Source {
	type Error;

	/// Adds the source to the drum.
	///
	/// # Errors
	///
	/// Implementations may return an error if the source could not be added to the drum.
	fn add_to_drum(self, drum: &mut DrumBuilder) -> Result<(), Self::Error>;
}

#[cfg(feature = "gltf")]
pub use gltf::GltfSource;

#[cfg(feature = "obj")]
pub use obj::ObjSource;
