use std::{
	collections::BTreeMap,
	io,
	net::TcpStream,
	sync::{
		mpsc::{self, TryRecvError},
		Arc, RwLock,
	},
};

use crate::{
	packet::{self, Packet, TrustedPacket},
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
	}

	fn broadcast_clients(
		&self,
		packet: TrustedPacket<Message>,
		clients: &mut BTreeMap<u32, TcpStream>,
	) {
		packet
			.write_iter(clients.iter_mut().map(|(_, stream)| stream))
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
					Packet::CreateInstance { options, .. } => {
						let id = self.next_instance_id();

						self.owners.insert(id, client_id);
						self.broadcast(
							Packet::CreateInstance { options, id }.into_trusted(client_id),
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
					Packet::CreateClient { instance_id } => {
						self.broadcast(
							Packet::CreateClient { instance_id }.into_trusted(client_id),
						);
					}
					Packet::DeleteClient => {
						self.owners.remove(&client_id);
						self.broadcast(Packet::DeleteClient.into_trusted(client_id));
					}
					Packet::Custom(message) => {
						self.broadcast(Packet::Custom(message).into_trusted(client_id));
					}
				},
				Err(TryRecvError::Empty) => {}
				Err(TryRecvError::Disconnected) => break,
			}

			let mut disconnected = Vec::new();
			let mut packets = Vec::new();

			// then, get any pending packets from clients
			for (client_id, stream) in self.clients.write().unwrap().iter_mut() {
				let packet = match Packet::read(stream) {
					Ok(packet) => packet,
					Err(packet::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => continue,
					Err(packet::Error::Io(..)) => {
						eprintln!("Client {} disconnected", client_id);
						disconnected.push(*client_id);
						continue;
					}
					Err(e) => {
						continue;
					}
				};

				let packet = packet.into_trusted(*client_id);

				match packet.inner {
					Packet::CreateInstance { .. } => {
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
						packets.push(packet);
					}
					Packet::UpdateInstance { id, .. } => {
						if self.owners.get(&id) != Some(client_id) {
							continue;
						}

						packets.push(packet);
					}
					Packet::CreateClient { .. } => {
						packets.push(packet);
					}
					Packet::DeleteClient => {
						self.owners.retain(|_, owner| owner != client_id);

						packets.push(packet);
					}
					Packet::Custom(..) => {
						packets.push(packet);
					}
				}
			}

			{
				let mut clients = self.clients.write().unwrap();

				for client_id in disconnected {
					clients.remove(&client_id);
				}

				for packet in packets {
					self.broadcast_clients(packet, &mut clients);
				}
			}
		}
	}

	fn run_client<A: App>(self) {
		let mut stream = A::connect();

		loop {
			// read from local
			if let Ok(packet) = self.packet_rx.try_recv() {
				packet.write(&mut stream).unwrap();
			}

			// read from server
			if let Ok(packet) = TrustedPacket::read(&mut stream) {
				self.packet_tx.send(packet).unwrap();
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
