#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
#[cfg(feature = "deepgram")]
pub mod deepgram_transcriber;
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
#[cfg(feature = "deepgram")]
pub use deepgram_transcriber::*;

#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa")))]
#[cfg(feature = "candle")]
pub mod whisper_transcriber;
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa")))]
#[cfg(feature = "candle")]
pub use whisper_transcriber::*;

#[cfg(target_arch = "wasm32")]
pub mod web_speech_transcriber;
#[cfg(target_arch = "wasm32")]
pub use web_speech_transcriber::*;

use bytes::Bytes;
use crossbeam::channel::RecvError;
use std::error::Error;
//use futures::channel::mpsc::{self, Sender, Receiver};
use anyhow::Result;
use futures::channel::mpsc;
use tokio::sync::broadcast::{self, Receiver, Sender};

use async_trait::async_trait;
use common::prelude::*;

use rust_decimal::prelude::*;
//use async_channel::{Sender, Receiver};

pub type result<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub type transcriber_sender = Sender<Bytes>;
pub type transcriber_receiver = Receiver<String>;

pub fn channel() -> (Sender<Bytes>, Receiver<Bytes>) {
    broadcast::channel(64)
}

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe_stream(
        &mut self,
        sample_rate: u32,
        stream: Receiver<Bytes>,
        token: CancellationToken,
    ) -> Result<mpsc::UnboundedReceiver<Result<TranscriptionResponse>>>;
}

#[derive(Clone, Default)]
pub struct TranscriptionResponse {
    pub speaker: Option<i32>,
    pub transcript: String,
    pub estimated_cost: Decimal,
    pub usage_quantity: f32,
    pub usage_unit: String,
}
