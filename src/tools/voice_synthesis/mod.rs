#![allow(warnings)]
use anyhow::Result;
use bytes::Bytes;
use std::collections::HashMap;

#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
pub mod azure_synthesizer;
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
pub mod eleven_labs_synthesizer;
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
#[cfg(feature = "openai")]
pub mod openai_synthesizer;
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
pub mod play_ht_synthesizer;

// Will need to likely add WASM support to 'hf_hub' crate for this
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
#[cfg(feature = "candle")]
pub mod candle_synthesizer;

#[cfg(all(
    feature = "sherpa",
    not(target_arch = "wasm32"),
    not(target_arch = "xtensa")
))]
pub mod sherpa_synthesizer;

pub mod coqui_synthesizer;

use async_trait::async_trait;
use rust_decimal::prelude::*;

#[derive(Default)]
pub struct SynthesisResult {
    pub bytes: Vec<u8>,
    pub cost: Decimal,
    pub usage_quantity: f32,
    pub usage_unit: String,
}

#[async_trait]
pub trait Synthesizer: Send + Sync {
    async fn create_speech_stream(
        &self,
        emotion: String,
        voice_name: String,
        text: String,
    ) -> Result<Option<SynthesisStream>> {
        Ok(None)
    }

    async fn create_speech(
        &self,
        emotion: String,
        voice_name: String,
        text: String,
    ) -> Result<SynthesisResult>;
}

pub struct SynthesisStream {
    pub format: delune::AudioFormat,
    pub chunks: tokio::sync::mpsc::Receiver<Vec<i16>>,
    pub completion: tokio::sync::oneshot::Receiver<Result<SynthesisResult>>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_chunks_preserve_order_and_tail() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let producer = std::thread::spawn(move || {
            sender.blocking_send(vec![1i16, 2]).unwrap();
            sender.blocking_send(vec![3i16]).unwrap();
        });
        assert_eq!(receiver.blocking_recv(), Some(vec![1, 2]));
        assert_eq!(receiver.blocking_recv(), Some(vec![3]));
        assert_eq!(receiver.blocking_recv(), None);
        producer.join().unwrap();
    }

    #[test]
    fn dropping_receiver_releases_backpressured_producer() {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        sender.try_send(vec![1i16]).unwrap();
        let producer = std::thread::spawn(move || sender.blocking_send(vec![2i16]));
        drop(receiver);
        assert!(producer.join().unwrap().is_err());
    }
}

pub mod prelude {
    #[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
    pub use super::azure_synthesizer::*;
    #[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
    pub use super::eleven_labs_synthesizer::*;
    #[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
    #[cfg(feature = "openai")]
    pub use super::openai_synthesizer::*;
    #[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
    pub use super::play_ht_synthesizer::*;
    #[cfg(all(
        feature = "sherpa",
        not(target_arch = "wasm32"),
        not(target_arch = "xtensa")
    ))]
    pub use super::sherpa_synthesizer::*;
    pub use super::*;
}
