use std::{collections::BTreeMap, net::TcpStream, sync::mpsc};

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

struct ServerState<Message> {
	message_rx: mpsc::Receiver<Packet<Message>>,
	packet_tx: mpsc::Sender<Packet<Message>>,

	clients: BTreeMap<u32, TcpStream>,
	next_client_id: u32,
}

impl<Message> ServerState<Message> {
	fn new(
		message_rx: mpsc::Receiver<Packet<Message>>,
		packet_tx: mpsc::Sender<Packet<Message>>,
	) -> Self {
		Self {
			message_rx,
			packet_tx,
			clients: BTreeMap::new(),
			next_client_id: 0,
		}
	}

	fn run(&mut self) {
		loop {
			match self.message_rx.try_recv() {
				Ok(message) => {
					let packet = Packet::new(message);

					for client_id in self.clients.keys() {
						self.packet_tx.send(packet.clone()).unwrap();
					}
				}
				Err(mpsc::TryRecvError::Empty) => {}
				Err(mpsc::TryRecvError::Disconnected) => break,
			}
		}
	}
}
