use std::{
	collections::BTreeMap,
	net::TcpStream,
	sync::{mpsc, Arc, RwLock},
};

use crate::{
	packet::{Packet, TrustedPacket},
	App, Context,
};

/// Hosts a server that can be connected to by clients.
///
/// If "client" and "server" features are enabled
struct Server<Message> {
	packet_rx: mpsc::Receiver<Packet<Message>>,
	packet_tx: mpsc::Sender<TrustedPacket<Message>>,

	clients: Arc<RwLock<BTreeMap<u32, TcpStream>>>,
	// represents the owner (client_id) of an instance
	owners: BTreeMap<u32, u32>,

	next_instance_id: u32,
	next_client_id: u32,
}

impl<Message> Server<Message>
where
	Message: bitcode::DecodeOwned + bitcode::Encode,
{
	fn new(
		packet_rx: mpsc::Receiver<Packet<Message>>,
		packet_tx: mpsc::Sender<TrustedPacket<Message>>,
	) -> Self {
		Self {
			packet_rx,
			packet_tx,
			clients: Arc::new(RwLock::new(BTreeMap::new())),
			owners: BTreeMap::new(),
			next_instance_id: 0,
			next_client_id: 0,
		}
	}

	/// Spawns a new thread to listen for incoming connections.
	fn run_listener<A: App<Message>>(&self) {
		let listener = A::listen();
		let clients = Arc::clone(&self.clients);

		let mut next_client_id = 0;

		std::thread::spawn(move || loop {
			// try to get another connecting client
			if let Ok((stream, _)) = listener.accept() {
				clients.write().unwrap().insert(next_client_id, stream);
				next_client_id += 1;
			}
		});
	}

	fn broadcast(&self, packet: TrustedPacket<Message>) {
		packet
			.write_iter(self.clients.write().unwrap().values_mut())
			.unwrap();

		self.packet_tx.send(packet).unwrap();
	}

	fn next_client_id(&mut self) -> u32 {
		let id = self.next_client_id;
		self.next_client_id += 1;
		id
	}

	fn next_instance_id(&mut self) -> u32 {
		let id = self.next_instance_id;
		self.next_instance_id += 1;
		id
	}

	/// Runs a dedicated server. This is used when the "server" feature is enabled.
	///
	/// Packets received from `packet_rx` are converted into trusted packets, treating
	/// the local client as an authority.
	fn run_server(mut self) {
		let client_id = self.next_client_id();

		loop {
			// first, get a pending packet from the local client
			match self.packet_rx.try_recv() {
				Ok(packet) => match packet {
					Packet::CreateInstance(create_instance) => {
						let id = self.next_instance_id();

						self.owners.insert(id, client_id);
						self.broadcast(
							Packet::CreateInstance(create_instance).into_trusted(client_id),
						);
					}
					Packet::DeleteInstance { id } => {
						self.owners.remove(&id);
						self.broadcast(Packet::DeleteInstance { id }.into_trusted(client_id));
					}
					Packet::UpdateInstance { id, delta } => {
						self.broadcast(
							Packet::UpdateInstance { id, delta }.into_trusted(client_id),
						);
					}
					Packet::CreateClient => {
						self.broadcast(Packet::CreateClient.into_trusted(client_id));
					}
					Packet::DeleteClient => {
						self.owners.remove(&client_id);
						self.broadcast(Packet::DeleteClient.into_trusted(client_id));
					}
					Packet::Custom(message) => {
						self.broadcast(Packet::Custom(message).into_trusted(client_id));
					}
				},
				Err(_) => break,
			}

			// then, get any pending packets from clients
			for (client_id, stream) in self.clients.write().unwrap().iter_mut() {
				let Ok(packet) = Packet::read(stream) else {
					continue;
				};

				let packet = packet.into_trusted(*client_id);

				match packet.inner {
					Packet::CreateInstance(..) => {
						let id = self.next_instance_id;
						self.next_instance_id += 1;

						self.owners.insert(id, *client_id);
						self.broadcast(packet);
					}
					Packet::DeleteInstance { id } => {
						if self.owners.get(&id) != Some(client_id) {
							continue;
						}

						self.owners.remove(&id);
						self.broadcast(packet);
					}
					Packet::UpdateInstance { id, .. } => {
						if self.owners.get(&id) != Some(client_id) {
							continue;
						}

						self.broadcast(packet);
					}
					Packet::CreateClient => {
						self.broadcast(packet);
					}
					Packet::DeleteClient => {
						self.owners.retain(|_, owner| owner != client_id);

						self.broadcast(packet);
					}
					Packet::Custom(..) => {
						self.broadcast(packet);
					}
				}
			}
		}
	}

	fn run_client<A: App>(self) {
		let mut stream = A::connect();

		loop {
			// read from local
			match self.packet_rx.try_recv() {
				Ok(packet) => {
					packet.write(&mut stream).unwrap();
				}
				Err(_) => {}
			}

			// read from server
			match TrustedPacket::read(&mut stream) {
				Ok(packet) => {
					self.packet_tx.send(packet).unwrap();
				}
				Err(_) => {}
			}
		}
	}
}

impl<Message> Context<Message> {
	pub fn send_packet(&self, packet: Packet<Message>) {
		self.packet_tx.send(packet).unwrap();
	}
}

pub fn run<A: App<Message>, Message>(
	packet_tx: mpsc::Sender<TrustedPacket<Message>>,
	packet_rx: mpsc::Receiver<Packet<Message>>,
) where
	Message: bitcode::DecodeOwned + bitcode::Encode,
{
	let state = Server::new(packet_rx, packet_tx);

	// Listens for new connections
	#[cfg(feature = "server")]
	Server::run_listener::<A>(&state);
	// Runs the server, with an attached client (either a "real" client
	// with rendering capabilities, or a "dummy" client for the server to
	// calculate physics)
	#[cfg(all(feature = "server", feature = "client"))]
	state.run_server();
	// Runs the client, which connects to a remove server.
	#[cfg(all(not(feature = "server"), feature = "client"))]
	Server::run_client::<A>(state);
}
