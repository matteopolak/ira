use crate::Context;

/// Implemented by the client to handle network events.
pub trait Client<Message> {
	/// Called when a client connects to the server.
	fn on_client_connect(&mut self, ctx: &mut Context<Message>, client_id: u32) {}
	/// Called when a client disconnects from the server.
	fn on_client_disconnect(&mut self, ctx: &mut Context<Message>, client_id: u32) {}
	/// Called when a custom [`Message`] is received from the server.
	fn on_message(&mut self, ctx: &mut Context<Message>, message: Message) {}
}
