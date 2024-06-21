use std::{
	collections::BTreeMap,
	net::TcpStream,
	sync::{mpsc, Arc, RwLock},
};

use crate::{packet::Packet, Context};

/// Implemented by the server to handle network events.
pub trait Server<Message> {
	/// Called when a client connects to the server.
	fn on_client_connect(&mut self, ctx: &mut Context<Message>, client_id: u32) {}
	/// Called when a client disconnects from the server.
	fn on_client_disconnect(&mut self, ctx: &mut Context<Message>, client_id: u32) {}
	/// Called when a custom [`Message`] is received from a client.
	fn on_message(&mut self, ctx: &mut Context<Message>, client_id: u32, message: Message) {}
}

/// Used to represent the state of the server.
///
/// If the "server" feature is not enabled, this will communicate with a remote server.
/// If the "server" feature is enabled, it will act as a server. If the "client" feature
/// is also enabled, the client will be an authority.
struct ServerState<Message> {
	packet_rx: mpsc::Receiver<Packet<Message>>,
	packet_tx: mpsc::Sender<Packet<Message>>,

	clients: Arc<RwLock<BTreeMap<u32, TcpStream>>>,
	// represents the owner (client_id) of an instance
	owners: BTreeMap<u32, u32>,

	next_instance_id: u32,
}

impl<Message> ServerState<Message>
where
	Message: bitcode::DecodeOwned + bitcode::Encode,
{
	fn new(
		packet_rx: mpsc::Receiver<Packet<Message>>,
		packet_tx: mpsc::Sender<Packet<Message>>,
	) -> Self {
		Self {
			packet_rx,
			packet_tx,
			clients: Arc::new(RwLock::new(BTreeMap::new())),
			owners: BTreeMap::new(),
			next_instance_id: 0,
		}
	}

	/// Spawns a new thread to listen for incoming connections.
	fn run_listener(&mut self) {
		let listener =
			std::net::TcpListener::bind("127.0.0.1:12345").expect("failed to bind to address");

		let mut next_client_id = 0;
		let clients = Arc::clone(&self.clients);

		std::thread::spawn(move || loop {
			// try to get another connecting client
			if let Ok((stream, _)) = listener.accept() {
				clients.write().unwrap().insert(next_client_id, stream);
				next_client_id += 1;
			}
		});
	}

	fn broadcast(&self, packet: Packet<Message>) {
		packet
			.write_iter(self.clients.write().unwrap().values_mut())
			.unwrap();
	}

	fn run(&mut self) {
		loop {
			match self.packet_rx.recv() {
				Ok(packet) => match packet {
					Packet::CreateInstance(create_instance) => {
						let id = self.next_instance_id;
						self.next_instance_id += 1;

						self.owners.insert(id, 0);
						self.broadcast(Packet::CreateInstance(create_instance));
					}
					Packet::DeleteInstance { id } => {
						self.owners.remove(&id);
						self.broadcast(Packet::DeleteInstance { id });
					}
					Packet::UpdateInstance { id, delta } => {
						self.broadcast(Packet::UpdateInstance { id, delta });
					}
					Packet::UpdateClientId { id } => {
						self.owners.insert(id, 0);
						self.broadcast(Packet::UpdateClientId { id });
					}
					Packet::Custom(message) => {
						self.broadcast(Packet::Custom(message));
					}
				},
				Err(_) => break,
			}
		}
	}
}

pub fn run<Message>(
	packet_tx: mpsc::Sender<Packet<Message>>,
	packet_rx: mpsc::Receiver<Packet<Message>>,
) where
	Message: bitcode::DecodeOwned + bitcode::Encode,
{
	let mut state = ServerState::new(packet_rx, packet_tx);

	state.run_listener();
	state.run();
}
