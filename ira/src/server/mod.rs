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
	App,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, bitcode::Encode, bitcode::Decode)]
pub struct InstanceId(u32);

impl InstanceId {
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

	// Listens for new connections
	#[cfg(feature = "server")]
	Server::run_listener::<A>(&state);
	// Runs the server, with an attached client (either a "real" client
	// with rendering capabilities, or a "dummy" client for the server to
	// calculate physics)
	#[cfg(all(feature = "server", feature = "client"))]
	Server::run_server(state, local);
	// Runs the client, which connects to a remove server.
	#[cfg(all(not(feature = "server"), feature = "client"))]
	Server::run_client::<A>(state);
}
