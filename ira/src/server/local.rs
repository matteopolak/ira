use std::{fmt, io, sync::atomic::Ordering};

use tracing::{debug, error, info, warn};

use crate::{
	client::{Client, ClientId},
	packet::{self, CreateInstance, Packet, TrustedPacket},
	App,
};

use super::{InstanceId, Server};

impl<M> Server<M>
where
	M: bitcode::DecodeOwned + bitcode::Encode,
{
	/// Spawns a new thread to listen for incoming connections.
	pub(crate) fn run_listener<A: App<M>>(&self) {
		let listener = A::listen();
		let addr = listener.local_addr().unwrap();

		let client_tx = self.client_tx.clone();
		let mut next_client_id = ClientId::SERVER.next();

		info!(addr = %addr, "listening for incoming connections");

		std::thread::spawn(move || loop {
			// try to get another connecting client
			let Ok((stream, _)) = listener.accept() else {
				continue;
			};

			stream.set_nonblocking(true).unwrap();
			client_tx.send(Client::new(stream, next_client_id)).unwrap();
			next_client_id = next_client_id.next();
		});
	}

	fn broadcast(&mut self, packet: &TrustedPacket<M>)
	where
		M: fmt::Debug,
	{
		debug!(?packet, "sending packet to clients");

		packet.write_iter(self.clients.values_mut()).unwrap();
	}

	fn next_instance_id(&mut self) -> InstanceId {
		InstanceId::new(self.next_instance_id.fetch_add(1, Ordering::SeqCst))
	}

	fn process_new_connections(&mut self)
	where
		M: fmt::Debug,
	{
		while let Ok(mut client) = self.client_rx.try_recv() {
			let instance_id = self.next_instance_id();

			info!(client_id = ?client.id(), "client connected");

			Packet::<M>::Connected { instance_id }
				.into_trusted(client.id())
				.write(&mut client)
				.unwrap();

			// send packets to clients to announce new client
			let create = Packet::<M>::CreateClient { instance_id }.into_trusted(client.id());

			self.broadcast(&create);
			self.packet_tx.send(create).unwrap();

			// lock for as little time as possible, so just collect immediately
			let packets = self
				.instances
				.iter()
				.map(|(id, options)| Packet::<M>::CreateInstance {
					id: *id,
					options: options.clone(),
				})
				.collect::<Vec<_>>();

			for packet in packets {
				let packet = packet.into_trusted(ClientId::SERVER);

				packet.write(&mut client).unwrap();
			}

			self.owners.insert(instance_id, client.id());
			self.clients.insert(client.id(), client);
		}
	}

	fn process_local_packets(&mut self, client_id: ClientId)
	where
		M: fmt::Debug,
	{
		while let Ok(packet) = self.packet_rx.try_recv() {
			match &packet {
				Packet::CreateInstance { options, id } => {
					self.instances.insert(*id, options.clone());
					self.owners.insert(*id, client_id);
				}
				Packet::DeleteInstance { id } => {
					self.instances.remove(&id);
					self.owners.remove(&id);
				}
				Packet::UpdateInstance { id, delta } => {
					if let Some(instance) = self.instances.get_mut(&id) {
						instance.apply(&delta);
					};
				}
				Packet::DeleteClient => {
					self.owners.retain(|_, owner| *owner != client_id);
				}
				Packet::Custom(..) | Packet::CreateClient { .. } => {}
				Packet::Connected { .. } => {
					warn!("received Connected packet from client");
					continue;
				}
			}

			self.broadcast(&packet.into_trusted(client_id));
		}
	}

	/// Processes local packets and returns the (disconnected clients, (packet owner, packets to send to clients)).
	fn process_remote_packets(&mut self) -> (Vec<ClientId>, Vec<(ClientId, TrustedPacket<M>)>)
	where
		M: fmt::Debug,
	{
		let mut disconnected = Vec::new();
		let mut packets = Vec::new();

		// then, get any pending packets from clients
		for client in &mut self.clients.values_mut() {
			loop {
				let packet = client.try_read_packet(&self.next_instance_id, &mut self.owners);

				match packet {
					Ok(Some(mut packet)) => {
						let id = match &mut packet {
							Packet::CreateInstance { options, id } => {
								*id = InstanceId::new(
									self.next_instance_id.fetch_add(1, Ordering::SeqCst),
								);

								self.instances.insert(*id, options.clone());
								self.owners.insert(*id, client.id());

								ClientId::SERVER
							}
							Packet::DeleteInstance { id } => {
								if self.owners.get(id) != Some(&client.id()) {
									warn!(instance_id = ?id, client_id = ?client.id(), "client tried to delete instance they don't own");

									continue;
								}

								self.instances.remove(id);
								self.owners.remove(id);

								ClientId::SERVER
							}
							Packet::UpdateInstance { id, delta } => {
								if self.owners.get(id) != Some(&client.id()) {
									warn!(instance_id = ?id, client_id = ?client.id(), "client tried to update instance they don't own");

									continue;
								}

								if let Some(instance) = self.instances.get_mut(id) {
									instance.apply(delta);
								};

								client.id()
							}
							Packet::DeleteClient => {
								self.owners.retain(|_, owner| *owner != client.id());

								client.id()
							}
							Packet::Custom(..) | Packet::CreateClient { .. } => client.id(),
							Packet::Connected { .. } => {
								warn!("received Connected packet from client");

								continue;
							}
						};

						packets.push((id, packet.into_trusted(client.id())));
					}
					Ok(None) => {
						break;
					}
					Err(packet::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {
						break;
					}
					Err(packet::Error::Io(..)) => {
						info!(client_id = ?client.id(), "client disconnected");
						disconnected.push(client.id());
						break;
					}
					Err(e) => {
						error!(error = ?e, "error reading packet");
						break;
					}
				}
			}
		}

		(disconnected, packets)
	}

	/// Runs a dedicated server. This is used when the "server" feature is enabled.
	///
	/// Packets received from `packet_rx` are converted into trusted packets, treating
	/// the local client as an authority.
	pub(crate) fn run_server(mut self, local: CreateInstance)
	where
		M: fmt::Debug,
	{
		let client_id = ClientId::SERVER;

		#[cfg(feature = "client")]
		{
			let instance_id = self.next_instance_id();

			self.packet_tx
				.send(Packet::<M>::Connected { instance_id }.into_trusted(client_id))
				.unwrap();

			self.instances.insert(instance_id, local);
		}

		loop {
			// try getting new clients
			self.process_new_connections();

			// first, get a pending packet from the local client
			self.process_local_packets(client_id);

			let (disconnected, packets) = self.process_remote_packets();

			if !disconnected.is_empty() {
				self.owners
					.retain(|_, owner| disconnected.binary_search(owner).is_ok());
			}

			for client_id in disconnected {
				self.clients.remove(&client_id);
			}

			for (client_id, packet) in packets {
				debug!(?packet, "sending packet to other clients");

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
}
