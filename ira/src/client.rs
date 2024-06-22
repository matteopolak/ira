use std::{
	io,
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

pub struct Client {
	stream: TcpStream,
	id: ClientId,
}

impl Client {
	#[must_use]
	pub fn new(stream: TcpStream, id: ClientId) -> Self {
		Self { stream, id }
	}

	#[must_use]
	pub fn id(&self) -> ClientId {
		self.id
	}

	/// Tries to read a packet from the client.
	///
	/// # Errors
	///
	/// See [`Packet::read`].
	pub fn try_read_packet<Message>(
		&mut self,
		next_instance_id: &AtomicU32,
		owners: &mut Owners,
	) -> Result<Option<Packet<Message>>, packet::Error>
	where
		Message: bitcode::DecodeOwned + bitcode::Encode,
	{
		let packet = Packet::read(&mut self.stream)?;

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
