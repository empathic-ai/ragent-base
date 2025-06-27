use bytes::Bytes;
use async_trait::async_trait;
use tokio::sync::broadcast::{self, channel, Sender, Receiver};
use dyn_clone::DynClone;

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
#[cfg(feature = "openai")]
pub mod chatgpt_realtime;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
#[cfg(feature = "openai")]
pub use chatgpt_realtime::*;

#[derive(Clone)]
pub enum RealtimeEvent {
	Text(String),
	Audio(Vec<i16>),
	AudioEnd
}

#[async_trait]
pub trait Realtime: Send + Sync + DynClone {
    /// Sends a `RealtimeEvent` to the chat.
    async fn send(&self, event: RealtimeEvent);

    /// Receives a `RealtimeEvent` from the chat.
    async fn recv(&mut self) -> RealtimeEvent;

	fn get_receiver(&self) -> Receiver<RealtimeEvent>;
}
