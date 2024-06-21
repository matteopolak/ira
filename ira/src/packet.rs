use std::io::{self, Write};

use glam::{Quat, Vec3};

use crate::{physics::PhysicsState, Body, Instance};

// TODO: implement Display and Error
pub enum Error {
	Io(io::Error),
	Bitcode(bitcode::Error),
}

impl From<io::Error> for Error {
	fn from(e: io::Error) -> Self {
		Self::Io(e)
	}
}

impl From<bitcode::Error> for Error {
	fn from(e: bitcode::Error) -> Self {
		Self::Bitcode(e)
	}
}

#[derive(Debug, bitcode::Encode, bitcode::Decode)]
pub enum Packet<Message> {
	/// An instance has been created.
	CreateInstance(CreateInstance),
	/// An instance has been deleted.
	DeleteInstance { id: u32 },
	/// An instance has been updated.
	UpdateInstance { id: u32, delta: UpdateInstance },
	/// A new client has connected.
	CreateClient,
	/// A client has disconnected.
	DeleteClient,
	/// A custom message (application-defined)
	Custom(Message),
}

impl<Message> Packet<Message> {
	pub fn new(message: Message) -> Self {
		Self::Custom(message)
	}

	pub fn into_trusted(self, client_id: u32) -> TrustedPacket<Message> {
		TrustedPacket {
			client_id,
			inner: self,
		}
	}

	pub fn read<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Message: bitcode::DecodeOwned,
	{
		let mut len = [0; 4];
		reader.read_exact(&mut len)?;

		let len = u32::from_le_bytes(len) as usize;
		let mut data = vec![0; len];
		reader.read_exact(&mut data)?;

		Ok(bitcode::decode(&data)?)
	}

	pub fn write_iter<'w, W: Write + 'w>(
		&self,
		writer: impl IntoIterator<Item = &'w mut W>,
	) -> io::Result<()>
	where
		Message: bitcode::Encode,
	{
		let data = bitcode::encode(self);
		let len = (data.len() as u32).to_le_bytes();

		for writer in writer {
			writer.write_all(&len)?;
			writer.write_all(&data)?;
		}

		Ok(())
	}

	pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()>
	where
		Message: bitcode::Encode,
	{
		let data = bitcode::encode(self);
		let len = (data.len() as u32).to_le_bytes();

		writer.write_all(&len)?;
		writer.write_all(&data)
	}
}

// packet with client id
#[derive(Debug, bitcode::Encode, bitcode::Decode)]
pub struct TrustedPacket<Message> {
	pub client_id: u32,
	pub inner: Packet<Message>,
}

impl<Message> TrustedPacket<Message> {
	pub fn read<R: io::Read>(reader: &mut R) -> Result<Self, Error>
	where
		Message: bitcode::DecodeOwned,
	{
		let mut len = [0; 4];
		reader.read_exact(&mut len)?;

		let len = u32::from_le_bytes(len) as usize;
		let mut data = vec![0; len];
		reader.read_exact(&mut data)?;

		Ok(bitcode::decode(&data)?)
	}

	pub fn write_iter<'w, W: Write + 'w>(
		&self,
		writer: impl IntoIterator<Item = &'w mut W>,
	) -> io::Result<()>
	where
		Message: bitcode::Encode,
	{
		let data = bitcode::encode(self);
		let len = (data.len() as u32).to_le_bytes();

		for writer in writer {
			writer.write_all(&len)?;
			writer.write_all(&data)?;
		}

		Ok(())
	}

	pub fn write<W: Write>(&self, writer: &mut W) -> io::Result<()>
	where
		Message: bitcode::Encode,
	{
		let data = bitcode::encode(self);
		let len = (data.len() as u32).to_le_bytes();

		writer.write_all(&len)?;
		writer.write_all(&data)
	}
}

/// A packet for creating a new collider.
#[derive(Debug, bitcode::Encode, bitcode::Decode)]
pub enum CreateCollider {
	Cuboid { half_extents: Vec3 },
	Sphere { radius: f32 },
}

#[derive(Debug, bitcode::Encode, bitcode::Decode)]
pub enum CreateBody {
	Static {
		position: Vec3,
		rotation: Quat,
	},
	Rigid {
		position: Vec3,
		rotation: Quat,
		velocity: Vec3,
		angular_velocity: Vec3,
	},
}

/// A packet for creating a new instance.
#[derive(Debug, bitcode::Encode, bitcode::Decode)]
pub struct CreateInstance {
	pub position: Vec3,
	pub rotation: Quat,
	pub scale: Vec3,
	pub body: CreateBody,
	pub collider: Option<CreateCollider>,
}

#[derive(Debug, bitcode::Encode, bitcode::Decode)]
pub enum UpdateBody {
	Static {
		position: Vec3,
		rotation: Quat,
	},
	Rigid {
		velocity: Vec3,
		angular_velocity: Vec3,
	},
}

/// A packet for updating an instance, providing only the changed fields.
#[derive(Debug, bitcode::Encode, bitcode::Decode)]
pub struct UpdateInstance {
	// These fields almost always change, so they are always included.
	pub position: Vec3,
	pub rotation: Quat,

	// These fields are optional, and are only included if they change.
	pub scale: Option<Vec3>,
	pub body: Option<UpdateBody>,
}

impl UpdateInstance {
	pub fn apply(&self, physics: &mut PhysicsState, instance: &mut Instance) {
		if let Some(scale) = self.scale {
			instance.scale = scale;
		}

		match &mut instance.body {
			Body::Static { position, rotation } => {
				*position = self.position;
				*rotation = self.rotation;
			}
			Body::Rigid(body) => {
				let Some(rigidbody) = physics.rigid_bodies.get_mut(*body) else {
					return;
				};

				rigidbody.set_position((self.position, self.rotation).into(), true);

				if let Some(UpdateBody::Rigid {
					velocity,
					angular_velocity,
				}) = self.body
				{
					rigidbody.set_linvel(velocity.into(), true);
					rigidbody.set_angvel(angular_velocity.into(), true);
				};
			}
		}
	}
}
