use super::UserEventWorker;
use super::UserEvent;
use bevy_builder::database::Thing;
use tokio::sync::broadcast::{Sender, Receiver};
use bevy::prelude::*;
use anyhow::Result;

#[cfg_attr(feature = "bevy", derive(Component))]
pub struct ConverterWorker {
	pub space_id: Thing,
	pub output_tx: Sender<UserEvent>,
    pub output_rx: Receiver<UserEvent>,
    pub input_tx: Sender<UserEvent>,
    pub input_rx: Receiver<UserEvent>,
}

impl ConverterWorker {
	pub fn new(space_id: Thing) -> Self {

		Self {
			space_id: space_id,
			output_tx: todo!(),
			output_rx: todo!(),
			input_tx: todo!(),
			input_rx: todo!(),
		}
	}
}

impl UserEventWorker for ConverterWorker {
	fn is_valid_space(&self, space_id: &Thing) -> Result<bool> {
		Ok(self.space_id == space_id.clone())
	}

	fn send_event(&mut self, ev: UserEvent) -> anyhow::Result<()> {
		todo!()
	}

	fn try_recv_event(&mut self) -> anyhow::Result<UserEvent> {
		todo!()
	}
}


