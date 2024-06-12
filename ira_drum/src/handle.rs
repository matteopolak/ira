use std::marker;

use bincode::{
	de::{BorrowDecoder, Decoder},
	enc::Encoder,
	error::{DecodeError, EncodeError},
	BorrowDecode, Decode, Encode,
};

#[derive(Debug, Clone, Copy)]
pub struct Handle<T> {
	index: u32,
	phantom: marker::PhantomData<T>,
}

impl<T> Handle<T> {
	#[must_use]
	pub fn new(index: u32) -> Self {
		Self {
			index,
			phantom: marker::PhantomData,
		}
	}

	pub fn from_vec(vec: &mut Vec<T>, new: T) -> Self {
		vec.push(new);
		Self::new(vec.len() as u32 - 1)
	}

	pub fn resolve<'d>(&self, bank: &'d [T]) -> &'d T {
		&bank[self.index as usize]
	}

	#[must_use]
	pub fn raw(&self) -> u32 {
		self.index
	}
}

impl<T> Encode for Handle<T> {
	fn encode<E: Encoder>(&self, encoder: &mut E) -> Result<(), EncodeError> {
		self.index.encode(encoder)
	}
}

impl<T> Decode for Handle<T> {
	fn decode<D: Decoder>(decoder: &mut D) -> Result<Self, DecodeError> {
		let index = u32::decode(decoder)?;

		Ok(Self::new(index))
	}
}

impl<'de, T> BorrowDecode<'de> for Handle<T> {
	fn borrow_decode<D: BorrowDecoder<'de>>(decoder: &mut D) -> Result<Self, DecodeError> {
		let index = u32::borrow_decode(decoder)?;

		Ok(Self::new(index))
	}
}
