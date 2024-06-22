use std::{fmt, io::BufReader, io::BufWriter};

use tracing::info;

use crate::{
	client::{Client, ClientId},
	packet::{Packet, TrustedPacket},
	App,
};

use super::Server;

impl<Message> Server<Message>
where
	Message: bitcode::DecodeOwned + bitcode::Encode,
{
	pub(crate) fn run_client<A: App<Message>>(self)
	where
		Message: fmt::Debug,
	{
		let stream = A::connect();
		let addr = stream.peer_addr().unwrap();

		info!(addr = %addr, "connected to server");

		let mut client = Client::new(stream, ClientId::SERVER);

		// get client id
		let packet = TrustedPacket::<Message>::read(&mut client).unwrap();
		let Packet::Connected { instance_id } = packet.inner else {
			panic!("expected SetClientId packet, got {packet:?}");
		};

		info!(
			client_id = ?packet.client_id,
			?instance_id, "received connection packet from server"
		);

		client.id = packet.client_id;

		client.stream.set_nonblocking(true).unwrap();
		self.packet_tx.send(packet).unwrap();

		loop {
			// read from local
			while let Ok(packet) = self.packet_rx.try_recv() {
				packet.write(&mut client).unwrap();
			}

			// read from server
			if let Ok(packet) = TrustedPacket::read(&mut client) {
				self.packet_tx.send(packet).unwrap();
			}
		}
	}
}
