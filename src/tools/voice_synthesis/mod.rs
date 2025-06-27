#![allow(warnings)]
use bytes::Bytes;
use anyhow::Result;
use std::collections::HashMap;

#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
#[cfg(feature = "openai")]
pub mod openai_synthesizer;
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
pub mod play_ht_synthesizer;
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
pub mod eleven_labs_synthesizer;
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
pub mod azure_synthesizer;

// Will need to likely add WASM support to 'hf_hub' crate for this
#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
#[cfg(feature = "candle")]
pub mod candle_synthesizer;

pub mod coqui_synthesizer;

use async_trait::async_trait;
use rust_decimal::prelude::*;

#[derive(Default)]
pub struct SynthesisResult {
    pub bytes: Vec<u8>,
    pub estimated_cost: Decimal
}

#[async_trait]
pub trait Synthesizer: Send + Sync {
    async fn create_speech(&self, emotion: String, voice_name: String, text: String) -> Result<SynthesisResult>;
}

pub mod prelude {
    #[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
    #[cfg(feature = "openai")]
    pub use super::openai_synthesizer::*;
    #[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
    pub use super::play_ht_synthesizer::*;
    #[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
    pub use super::eleven_labs_synthesizer::*;
    #[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa", target_os = "android")))]
    pub use super::azure_synthesizer::*;
    pub use super::*;
}