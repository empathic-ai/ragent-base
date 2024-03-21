#[cfg(not(target_arch = "wasm32"))]
pub mod deepgram_transcriber;
#[cfg(not(target_arch = "wasm32"))]
pub use deepgram_transcriber::*;

pub mod whisper_transcriber;
pub use whisper_transcriber::*;

use std::error::Error;
use bytes::Bytes;
use crossbeam::channel::RecvError;
//use futures::channel::mpsc::{self, Sender, Receiver};
use tokio::sync::broadcast::{self, Sender, Receiver};
use anyhow::Result;
use futures::channel::mpsc;

use async_trait::async_trait;
use common::prelude::*;
//use async_channel::{Sender, Receiver};

pub type result<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub type transcriber_sender = Sender<Bytes>;
pub type transcriber_receiver = Receiver<String>;

pub fn channel() -> (Sender<Bytes>, Receiver<Bytes>) {
    broadcast::channel(16)
}

#[async_trait]
pub trait Transcriber: Send + Sync {
    async fn transcribe_stream(&mut self, sample_rate: u32, stream: Receiver<Bytes>, token: CancellationToken) -> Result<mpsc::Receiver<Result<String>>>;
}
