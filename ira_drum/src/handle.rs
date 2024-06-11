use std::marker;

use bincode::{
	de::{BorrowDecoder, Decoder},
	enc::Encoder,
	error::{DecodeError, EncodeError},
	BorrowDecode, Decode, Encode,
};

#[must_use]
#[derive(Debug)]
pub struct Handle<T> {
	index: u32,
	phantom: marker::PhantomData<T>,
}

impl<T> Handle<T> {
	pub fn new(index: u32) -> Self {
		Self {
			index,
			phantom: marker::PhantomData,
		}
	}

	pub fn resolve<'d>(&self, bank: &'d [T]) -> &'d T {
		&bank[self.index as usize]
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
