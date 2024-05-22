#![allow(warnings)]
use bytes::Bytes;
use anyhow::Result;
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub mod openai_synthesizer;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub mod play_ht_synthesizer;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub mod eleven_labs_synthesizer;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub mod azure_synthesizer;

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
    #[cfg(not(target_arch = "xtensa"))]
    pub use super::openai_synthesizer::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(target_arch = "xtensa"))]
    pub use super::play_ht_synthesizer::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(target_arch = "xtensa"))]
    pub use super::eleven_labs_synthesizer::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(target_arch = "xtensa"))]
    pub use super::azure_synthesizer::*;
    pub use super::*;
}