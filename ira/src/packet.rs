use std::io::{self, Write};

use glam::{Quat, Vec3};
use rapier3d::{dynamics::RigidBodyBuilder, geometry::ColliderBuilder};

use crate::{
	client::{Client, ClientId},
	physics::PhysicsState,
	server::InstanceId,
	Body, Instance, InstanceBuilder,
};

// TODO: implement Display and Error
#[derive(Debug)]
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
	CreateInstance {
		options: CreateInstance,
		id: InstanceId,
	},
	/// An instance has been deleted.
	DeleteInstance { id: InstanceId },
	/// An instance has been updated.
	UpdateInstance {
		id: InstanceId,
		delta: UpdateInstance,
	},
	/// A new client has connected.
	CreateClient { instance_id: InstanceId },
	/// The first packet sent to a client, containing its own client id as the receiver
	/// of the wrapped [`TrustedPacket`], and its own instance id.
	Connected { instance_id: InstanceId },
	/// A client has disconnected.
	DeleteClient,
	/// A custom message (application-defined)
	Custom(Message),
}

impl<Message> Packet<Message> {
	/// Creates a new packet with custom message.
	pub fn new(message: Message) -> Self {
		Self::Custom(message)
	}

	/// Wraps this packet in a [`TrustedPacket`] with the given client id.
	pub fn into_trusted(self, client_id: ClientId) -> TrustedPacket<Message> {
		TrustedPacket {
			client_id,
			inner: self,
		}
	}

	/// Reads a packet from a reader.
	///
	/// # Errors
	///
	/// See [`bitcode::Error`] and [`io::Error`] for more information.
	pub fn read(client: &mut Client) -> Result<Self, Error>
	where
		Message: bitcode::DecodeOwned,
	{
		read(client)
	}

	/// Writes a packet to multiple writers.
	///
	/// # Errors
	///
	/// See [`bitcode::Error`] and [`io::Error`] for more information.
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

	/// Writes a packet to a writer.
	///
	/// # Errors
	///
	/// See [`bitcode::Error`] and [`io::Error`] for more information.
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
	pub client_id: ClientId,
	pub inner: Packet<Message>,
}

impl<Message> TrustedPacket<Message> {
	/// Reads a packet from a reader.
	///
	/// # Errors
	///
	/// See [`bitcode::Error`] and [`io::Error`] for more information.
	pub fn read(client: &mut Client) -> Result<Self, Error>
	where
		Message: bitcode::DecodeOwned,
	{
		read(client)
	}

	/// Writes a packet to multiple writers.
	///
	/// # Errors
	///
	/// See [`bitcode::Error`] and [`io::Error`] for more information.
	pub fn write_iter<'w>(&self, writer: impl IntoIterator<Item = &'w mut Client>) -> io::Result<()>
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

	/// Writes a packet to a writer.
	///
	/// # Errors
	///
	/// See [`bitcode::Error`] and [`io::Error`] for more information.
	pub fn write(&self, writer: &mut Client) -> io::Result<()>
	where
		Message: bitcode::Encode,
	{
		let data = bitcode::encode(self);
		let len = (data.len() as u32).to_le_bytes();

		writer.write_all(&len)?;
		writer.write_all(&data)
	}
}

fn read<T>(client: &mut Client) -> Result<T, Error>
where
	T: bitcode::DecodeOwned,
{
	let buf = client.next_packet()?;

	Ok(bitcode::decode(&buf)?)
}

/// A packet for creating a new collider.
#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub enum CreateCollider {
	Cuboid {
		half_extents: Vec3,
	},
	Sphere {
		radius: f32,
	},
	Capsule {
		segment_a: Vec3,
		segment_b: Vec3,
		radius: f32,
	},
	Cylinder {
		half_height: f32,
		radius: f32,
	},
	Cone {
		half_height: f32,
		radius: f32,
	},
}

impl CreateCollider {
	#[must_use]
	pub fn from_builder(builder: &ColliderBuilder) -> Self {
		let ball = builder.shape.as_ball();

		if let Some(ball) = ball {
			return Self::Sphere {
				radius: ball.radius,
			};
		}

		let cuboid = builder.shape.as_cuboid();

		if let Some(cuboid) = cuboid {
			return Self::Cuboid {
				half_extents: cuboid.half_extents.into(),
			};
		}

		let capsule = builder.shape.as_capsule();

		if let Some(capsule) = capsule {
			return Self::Capsule {
				segment_a: capsule.segment.a.into(),
				segment_b: capsule.segment.b.into(),
				radius: capsule.radius,
			};
		}

		let cylinder = builder.shape.as_cylinder();

		if let Some(cylinder) = cylinder {
			return Self::Cylinder {
				half_height: cylinder.half_height,
				radius: cylinder.radius,
			};
		}

		let cone = builder.shape.as_cone();

		if let Some(cone) = cone {
			return Self::Cone {
				half_height: cone.half_height,
				radius: cone.radius,
			};
		}

		todo!()
	}
}

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub enum CreateBody {
	Static,
	Rigid {
		kind: RigidBodyType,
		velocity: Vec3,
		angular_velocity: Vec3,
	},
}

/// A packet for creating a new instance.
#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub struct CreateInstance {
	pub model_id: u32,
	pub position: Vec3,
	pub rotation: Quat,
	pub scale: Vec3,
	pub body: CreateBody,
	pub collider: Option<CreateCollider>,
}

#[derive(Debug, Clone, bitcode::Encode, bitcode::Decode)]
pub enum RigidBodyType {
	Dynamic = 0,
	Fixed = 1,
	KinematicPositionBased = 2,
	KinematicVelocityBased = 3,
}

impl From<RigidBodyType> for rapier3d::dynamics::RigidBodyType {
	fn from(kind: RigidBodyType) -> Self {
		match kind {
			RigidBodyType::Dynamic => Self::Dynamic,
			RigidBodyType::Fixed => Self::Fixed,
			RigidBodyType::KinematicPositionBased => Self::KinematicPositionBased,
			RigidBodyType::KinematicVelocityBased => Self::KinematicVelocityBased,
		}
	}
}

impl From<rapier3d::dynamics::RigidBodyType> for RigidBodyType {
	fn from(kind: rapier3d::dynamics::RigidBodyType) -> Self {
		match kind {
			rapier3d::dynamics::RigidBodyType::Dynamic => Self::Dynamic,
			rapier3d::dynamics::RigidBodyType::Fixed => Self::Fixed,
			rapier3d::dynamics::RigidBodyType::KinematicPositionBased => {
				Self::KinematicPositionBased
			}
			rapier3d::dynamics::RigidBodyType::KinematicVelocityBased => {
				Self::KinematicVelocityBased
			}
		}
	}
}

impl CreateInstance {
	pub fn apply(&mut self, delta: &UpdateInstance) {
		self.position = delta.position;
		self.rotation = delta.rotation;

		if let Some(scale) = delta.scale {
			self.scale = scale;
		}

		if let Some(body) = &delta.body {
			match (&mut self.body, body) {
				(CreateBody::Static, UpdateBody::Static { .. }) => {
					self.body = CreateBody::Static;
				}
				(
					CreateBody::Rigid {
						velocity,
						angular_velocity,
						..
					},
					UpdateBody::Rigid {
						velocity: new_velocity,
						angular_velocity: new_angular_velocity,
					},
				) => {
					*velocity = *new_velocity;
					*angular_velocity = *new_angular_velocity;
				}
				_ => todo!(),
			}
		}
	}

	pub fn into_builder(self) -> InstanceBuilder {
		let instance = Instance::builder()
			.position(self.position)
			.rotation(self.rotation)
			.scale(self.scale);

		let instance = match self.collider {
			None => instance,
			Some(collider) => instance.collider(match collider {
				CreateCollider::Cuboid { half_extents } => {
					ColliderBuilder::cuboid(half_extents.x, half_extents.y, half_extents.z)
				}
				CreateCollider::Sphere { radius } => ColliderBuilder::ball(radius),
				CreateCollider::Capsule {
					segment_a,
					segment_b,
					radius,
				} => ColliderBuilder::capsule_from_endpoints(
					segment_a.into(),
					segment_b.into(),
					radius,
				),
				CreateCollider::Cylinder {
					half_height,
					radius,
				} => ColliderBuilder::cylinder(half_height, radius),
				CreateCollider::Cone {
					half_height,
					radius,
				} => ColliderBuilder::cone(half_height, radius),
			}),
		};

		match self.body {
			CreateBody::Static => instance,
			CreateBody::Rigid {
				velocity,
				angular_velocity,
				kind,
			} => instance.rigidbody(
				RigidBodyBuilder::new(kind.into())
					.linvel(velocity.into())
					.angvel(angular_velocity.into()),
			),
		}
	}

	pub fn from_builder(builder: &InstanceBuilder, model_id: u32) -> Self {
		let (position, rotation) = (builder.position, builder.rotation);
		let scale = builder.scale;

		let body = builder
			.rigidbody
			.as_ref()
			.map_or(CreateBody::Static, |body| CreateBody::Rigid {
				velocity: body.linvel.into(),
				angular_velocity: body.angvel.into(),
				kind: body.body_type.into(),
			});

		let collider = builder.collider.as_ref().map(CreateCollider::from_builder);

		Self {
			model_id,
			position,
			rotation,
			scale,
			body,
			collider,
		}
	}
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
