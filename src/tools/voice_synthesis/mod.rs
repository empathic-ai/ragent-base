#![allow(warnings)]
use bytes::Bytes;
use anyhow::Result;
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(feature = "xtensa"))]
pub mod openai_tts;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(feature = "xtensa"))]
pub mod play_ht;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(feature = "xtensa"))]
pub mod eleven_labs;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(feature = "xtensa"))]
pub mod azure_tts;

pub mod piper_synthesizer;
pub mod coqui_synthesizer;

use async_trait::async_trait;

pub struct SynthesisResult {
    pub bytes: Vec<u8>
}

#[async_trait]
pub trait Synthesizer: Send + Sync {
    async fn create_speech(&self, emotion: String, voice_name: String, text: String) -> Result<SynthesisResult>;
}

pub mod prelude {
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(feature = "xtensa"))]
    pub use super::openai_tts::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(feature = "xtensa"))]
    pub use super::play_ht::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(feature = "xtensa"))]
    pub use super::eleven_labs::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(feature = "xtensa"))]
    pub use super::azure_tts::*;
    pub use super::*;
}