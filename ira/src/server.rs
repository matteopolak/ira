use std::{
	collections::BTreeMap,
	fmt, io,
	net::TcpStream,
	sync::{
		atomic::{AtomicU32, Ordering},
		mpsc::{self, TryRecvError},
		Arc,
	},
};

use tracing::{error, info, warn};

use crate::{
	packet::{self, CreateInstance, Packet, TrustedPacket},
	App, Context,
};

/// Hosts a server that can be connected to by clients.
///
/// If "client" and "server" features are enabled
struct Server<Message> {
	packet_rx: mpsc::Receiver<Packet<Message>>,
	packet_tx: mpsc::Sender<TrustedPacket<Message>>,

	client_tx: mpsc::Sender<TcpStream>,
	client_rx: mpsc::Receiver<TcpStream>,

	clients: BTreeMap<u32, TcpStream>,
	// an up-to-date list of all active instances, indexed by their id
	instances: BTreeMap<u32, CreateInstance>,
	// represents the owner (client_id) of an instance
	owners: BTreeMap<u32, u32>,

	next_instance_id: Arc<AtomicU32>,
	next_client_id: u32,
}

impl<Message> Server<Message>
where
	Message: bitcode::DecodeOwned + bitcode::Encode,
{
	fn new(
		packet_rx: mpsc::Receiver<Packet<Message>>,
		packet_tx: mpsc::Sender<TrustedPacket<Message>>,
		client_tx: mpsc::Sender<TcpStream>,
		client_rx: mpsc::Receiver<TcpStream>,
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
			next_client_id: 0,
		}
	}

	/// Spawns a new thread to listen for incoming connections.
	fn run_listener<A: App<Message>>(&self) {
		let listener = A::listen();
		let addr = listener.local_addr().unwrap();

		let client_tx = self.client_tx.clone();

		info!(addr = %addr, "listening for incoming connections");

		std::thread::spawn(move || loop {
			// try to get another connecting client
			let Ok((stream, _)) = listener.accept() else {
				continue;
			};

			stream.set_nonblocking(true).unwrap();

			client_tx.send(stream).unwrap();
		});
	}

	fn broadcast(&mut self, packet: &TrustedPacket<Message>)
	where
		Message: fmt::Debug,
	{
		info!(packet = ?packet, "sending packet to clients");

		packet.write_iter(self.clients.values_mut()).unwrap();
	}

	fn next_client_id(&mut self) -> u32 {
		let id = self.next_client_id;
		self.next_client_id += 1;
		id
	}

	fn next_instance_id(&mut self) -> u32 {
		self.next_instance_id.fetch_add(1, Ordering::SeqCst)
	}

	/// Runs a dedicated server. This is used when the "server" feature is enabled.
	///
	/// Packets received from `packet_rx` are converted into trusted packets, treating
	/// the local client as an authority.
	#[allow(clippy::too_many_lines)]
	fn run_server(mut self, local: CreateInstance)
	where
		Message: fmt::Debug,
	{
		let client_id = self.next_client_id();

		#[cfg(feature = "client")]
		{
			let instance_id = self.next_instance_id();

			self.packet_tx
				.send(Packet::<Message>::Connected { instance_id }.into_trusted(client_id))
				.unwrap();

			self.instances.insert(instance_id, local);
		}

		loop {
			// try getting new clients
			while let Ok(mut client) = self.client_rx.try_recv() {
				let client_id = self.next_client_id();
				let instance_id = self.next_instance_id();

				Packet::<Message>::Connected { instance_id }
					.into_trusted(client_id)
					.write(&mut client)
					.unwrap();

				// send packets to clients to announce new client
				let create =
					Packet::<Message>::CreateClient { instance_id }.into_trusted(client_id);

				self.broadcast(&create);
				self.packet_tx.send(create).unwrap();

				// lock for as little time as possible, so just collect immediately
				let packets = self
					.instances
					.iter()
					.map(|(id, options)| Packet::<Message>::CreateInstance {
						id: *id,
						options: options.clone(),
					})
					.collect::<Vec<_>>();

				for packet in packets {
					let packet = packet.into_trusted(0);

					packet.write(&mut client).unwrap();
				}

				self.clients.insert(client_id, client);
				self.owners.insert(instance_id, client_id);
			}

			// first, get a pending packet from the local client
			match self.packet_rx.try_recv() {
				Ok(packet) => match packet {
					Packet::CreateInstance { options, id } => {
						self.instances.insert(id, options.clone());

						self.owners.insert(id, client_id);
						self.broadcast(
							&Packet::CreateInstance { options, id }.into_trusted(client_id),
						);
					}
					Packet::DeleteInstance { id } => {
						self.instances.remove(&id);

						self.owners.remove(&id);
						self.broadcast(&Packet::DeleteInstance { id }.into_trusted(client_id));
					}
					Packet::UpdateInstance { id, delta } => {
						if let Some(instance) = self.instances.get_mut(&id) {
							instance.apply(&delta);
						};

						self.broadcast(
							&Packet::UpdateInstance { id, delta }.into_trusted(client_id),
						);
					}
					Packet::CreateClient { instance_id } => {
						self.broadcast(
							&Packet::CreateClient { instance_id }.into_trusted(client_id),
						);
					}
					Packet::DeleteClient => {
						self.owners.remove(&client_id);
						self.broadcast(&Packet::DeleteClient.into_trusted(client_id));
					}
					Packet::Custom(message) => {
						self.broadcast(&Packet::Custom(message).into_trusted(client_id));
					}
					Packet::Connected { .. } => {
						warn!("received Connected packet from client");
					}
				},
				Err(TryRecvError::Empty) => {}
				Err(TryRecvError::Disconnected) => break,
			}

			let mut disconnected = Vec::new();
			let mut packets = Vec::new();

			// then, get any pending packets from clients
			for (&client_id, stream) in &mut self.clients {
				loop {
					let packet = match Packet::read(stream) {
						Ok(packet) => packet,
						Err(packet::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {
							break;
						}
						Err(packet::Error::Io(..)) => {
							eprintln!("Client {client_id} disconnected");
							disconnected.push(client_id);
							break;
						}
						Err(e) => {
							error!(error = ?e, "error reading packet");

							break;
						}
					};

					let packet = packet.into_trusted(client_id);

					info!(packet = ?packet, "received packet from client");

					match packet.inner {
						Packet::CreateInstance { .. } => {
							let id = self.next_instance_id.fetch_add(1, Ordering::SeqCst);

							self.owners.insert(id, client_id);
							packets.push((client_id, packet));
						}
						Packet::DeleteInstance { id } => {
							if self.owners.get(&id) != Some(&client_id) {
								break;
							}

							self.owners.remove(&id);
							packets.push((client_id, packet));
						}
						Packet::UpdateInstance { id, .. } => {
							if self.owners.get(&id) != Some(&client_id) {
								break;
							}

							packets.push((client_id, packet));
						}
						Packet::CreateClient { .. } | Packet::Custom(..) => {
							packets.push((client_id, packet));
						}
						Packet::DeleteClient => {
							self.owners.retain(|_, owner| *owner != client_id);

							packets.push((client_id, packet));
						}
						Packet::Connected { .. } => {
							warn!("received Connected packet from self");
						}
					}
				}
			}

			if !disconnected.is_empty() {
				self.owners
					.retain(|_, owner| disconnected.binary_search(owner).is_ok());
			}

			for client_id in disconnected {
				self.clients.remove(&client_id);
			}

			for (client_id, packet) in packets {
				info!(packet = ?packet, "sending packet to other clients");

				packet
					.write_iter(
						self.clients
							.iter_mut()
							.filter_map(|(id, stream)| (client_id != *id).then_some(stream)),
					)
					.unwrap();

				self.packet_tx.send(packet).unwrap();
			}
		}
	}

	#[cfg(all(not(feature = "server"), feature = "client"))]
	fn run_client<A: App<Message>>(self)
	where
		Message: fmt::Debug,
	{
		let mut stream = A::connect();
		let addr = stream.peer_addr().unwrap();

		info!(addr = %addr, "connected to server");

		// get client id
		let packet = TrustedPacket::<Message>::read(&mut stream).unwrap();
		let Packet::Connected { instance_id } = packet.inner else {
			panic!("expected SetClientId packet, got {packet:?}");
		};

		info!(
			client_id = packet.client_id,
			instance_id, "received connection packet from server"
		);

		stream.set_nonblocking(true).unwrap();
		self.packet_tx.send(packet).unwrap();

		loop {
			// read from local
			while let Ok(packet) = self.packet_rx.try_recv() {
				packet.write(&mut stream).unwrap();
			}

			// read from server
			if let Ok(packet) = TrustedPacket::read(&mut stream) {
				info!(packet = ?packet, "received packet from server");

				self.packet_tx.send(packet).unwrap();
			}
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
