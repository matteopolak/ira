use std::{
	io::{self, Read},
	mem,
	net::TcpStream,
	sync::atomic::{AtomicU32, Ordering},
};

use tracing::warn;

use crate::{
	packet::{self, Packet},
	server::{InstanceId, Owners},
};

#[derive(
	Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, bitcode::Encode, bitcode::Decode,
)]
pub struct ClientId(u32);

impl ClientId {
	pub const SERVER: ClientId = ClientId(0);

	#[must_use]
	pub fn next(&self) -> Self {
		Self(self.0 + 1)
	}
}

pub struct PartialPacket {
	pub(crate) len: Option<usize>,
	pub(crate) data: Vec<u8>,
	pub(crate) offset: usize,
}

impl Default for PartialPacket {
	fn default() -> Self {
		Self {
			len: None,
			data: vec![0; 4],
			offset: 0,
		}
	}
}

pub struct Client {
	pub(crate) stream: TcpStream,
	pub(crate) id: ClientId,

	/// A partial packet that is being read.
	///
	/// When reading the next packet, check if this is `Some`. If it is,
	/// read the remaining data into the partial packet.
	pub(crate) next_packet: PartialPacket,
}

impl Client {
	/// Creates a new client from a stream.
	///
	/// # Panics
	///
	/// Panics if the stream cannot be cloned.
	#[must_use]
	pub fn new(stream: TcpStream, id: ClientId) -> Self {
		Self {
			stream,
			id,
			next_packet: PartialPacket::default(),
		}
	}

	#[must_use]
	pub fn id(&self) -> ClientId {
		self.id
	}

	/// Tries to complete the current packet, returning
	/// the packet data if it completes.
	pub fn next_packet(&mut self) -> io::Result<Vec<u8>> {
		let mut partial = mem::take(&mut self.next_packet);

		let len = match partial.len {
			Some(len) => len,
			None => {
				while partial.offset < 4 {
					let n = match self.stream.read(&mut partial.data[partial.offset..4]) {
						Ok(n) => n,
						Err(e) => {
							self.next_packet = partial;
							return Err(e);
						}
					};

					partial.offset += n;
				}

				let len = u32::from_le_bytes([
					partial.data[0],
					partial.data[1],
					partial.data[2],
					partial.data[3],
				]) as usize;

				partial.len = Some(len);
				partial.data.resize(len, 0);
				partial.offset = 0;

				len
			}
		};

		while partial.offset < len {
			let n = match self.stream.read(&mut partial.data[partial.offset..len]) {
				Ok(n) => n,
				Err(e) => {
					self.next_packet = partial;
					return Err(e);
				}
			};

			partial.offset += n;
		}

		Ok(partial.data)
	}

	/// Tries to read a packet from the client.
	///
	/// # Errors
	///
	/// See [`Packet::read`].
	pub fn try_read_packet<M>(
		&mut self,
		next_instance_id: &AtomicU32,
		owners: &mut Owners,
	) -> Result<Option<Packet<M>>, packet::Error>
	where
		M: bitcode::DecodeOwned + bitcode::Encode,
	{
		let packet = Packet::read(self)?;

		match packet {
			Packet::CreateInstance { .. } => {
				let id = InstanceId::new(next_instance_id.fetch_add(1, Ordering::SeqCst));
				owners.insert(id, self.id);
			}
			Packet::DeleteInstance { id } => {
				if owners.get(&id) != Some(&self.id) {
					return Ok(None);
				}

				owners.remove(&id);
			}
			Packet::UpdateInstance { id, .. } => {
				if owners.get(&id) != Some(&self.id) {
					return Ok(None);
				}
			}
			Packet::DeleteClient => {
				owners.retain(|_, owner| *owner != self.id);
			}
			Packet::CreateClient { .. } | Packet::Custom(..) => {}
			Packet::Connected { .. } => {
				warn!("received Connected packet from self");
				return Ok(None);
			}
		};

		Ok(Some(packet))
	}
}

impl io::Write for Client {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.stream.write(buf)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.stream.flush()
	}
}

impl io::Read for Client {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		self.stream.read(buf)
	}
}
