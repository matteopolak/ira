#[cfg(feature = "server")]
pub mod local;
#[cfg(all(not(feature = "server"), feature = "client"))]
pub mod remote;

use std::{
	collections::BTreeMap,
	fmt,
	sync::{atomic::AtomicU32, mpsc, Arc},
};

use crate::{
	client::{Client, ClientId},
	packet::{CreateInstance, Packet, TrustedPacket},
	App, Context,
};

pub type Owners = BTreeMap<InstanceId, ClientId>;

/// Hosts a server that can be connected to by clients.
struct Server<Message> {
	packet_rx: mpsc::Receiver<Packet<Message>>,
	packet_tx: mpsc::Sender<TrustedPacket<Message>>,

	client_tx: mpsc::Sender<Client>,
	client_rx: mpsc::Receiver<Client>,

	clients: BTreeMap<ClientId, Client>,
	// an up-to-date list of all active instances, indexed by their id
	instances: BTreeMap<InstanceId, CreateInstance>,
	// represents the owner (client_id) of an instance
	owners: Owners,

	next_instance_id: Arc<AtomicU32>,
}

impl<Message> Server<Message> {
	pub(crate) fn new(
		packet_rx: mpsc::Receiver<Packet<Message>>,
		packet_tx: mpsc::Sender<TrustedPacket<Message>>,
		client_tx: mpsc::Sender<Client>,
		client_rx: mpsc::Receiver<Client>,
		next_instance_id: Arc<AtomicU32>,
	) -> Self {
		Self {
			packet_rx,
			packet_tx,
			client_tx,
			client_rx,
			clients: BTreeMap::new(),
			instances: BTreeMap::new(),
			owners: BTreeMap::new(),
			next_instance_id,
		}
	}
}

impl<Message> Context<Message> {
	/// Sends a packet to the server.
	///
	/// # Panics
	///
	/// Panics if the packet channel has closed, which will only happen if the
	/// communication with the server has failed.
	pub fn send_packet(&self, packet: Packet<Message>) {
		self.packet_tx.send(packet).unwrap();
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, bitcode::Encode, bitcode::Decode)]
pub struct InstanceId(u32);

impl InstanceId {
	pub const NONE: Self = Self(u32::MAX);

	#[must_use]
	pub fn new(id: u32) -> Self {
		Self(id)
	}
}

pub fn run<A: App<Message>, Message>(
	packet_tx: mpsc::Sender<TrustedPacket<Message>>,
	packet_rx: mpsc::Receiver<Packet<Message>>,
	next_instance_id: Arc<AtomicU32>,
	local: CreateInstance,
) where
	Message: bitcode::DecodeOwned + bitcode::Encode + fmt::Debug,
{
	let (client_tx, client_rx) = mpsc::channel();
	let state = Server::new(packet_rx, packet_tx, client_tx, client_rx, next_instance_id);

	#[cfg(feature = "server")]
	Server::run_listener::<A>(&state);
	#[cfg(feature = "server")]
	Server::run_server(state, local);
	// Runs the client, which connects to a remote server.
	#[cfg(all(not(feature = "server"), feature = "client"))]
	Server::run_client::<A>(state);
}
