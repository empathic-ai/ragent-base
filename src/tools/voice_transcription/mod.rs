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

use std::error::Error;
use bytes::Bytes;
use crossbeam::channel::RecvError;
//use futures::channel::mpsc::{self, Sender, Receiver};
use tokio::sync::broadcast::{self, Sender, Receiver};
use anyhow::Result;
use futures::channel::mpsc;

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
    async fn transcribe_stream(&mut self, sample_rate: u32, stream: Receiver<Bytes>, token: CancellationToken) -> Result<mpsc::UnboundedReceiver<Result<TranscriptionResponse>>>;
}

#[derive(Clone, Default)]
pub struct TranscriptionResponse {
    /// Deprecated numeric Deepgram label. Use `diarization_label` for new code.
    pub speaker: Option<i32>,
    /// Provider-scoped, session-local speaker label; never a durable identity.
    pub diarization_label: Option<String>,
    /// Unique connection id. Labels and relative timestamps reset on reconnect.
    pub session_id: Option<String>,
    /// Offset into the input PCM sample clock; None after unknown-duration loss.
    pub stream_start_sample: Option<u64>,
    pub transcript: String,
    pub estimated_cost: Decimal,
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub is_final: bool,
    pub speech_final: bool,
}
