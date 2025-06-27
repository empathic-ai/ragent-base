#![allow(warnings)]
use bytes::Bytes;
use anyhow::Result;
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
#[cfg(not(target_os = "android"))]
#[cfg(feature = "openai")]
pub mod dalle2_image_generator;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_os = "android"))]
#[cfg(feature = "candle")]
pub mod candle_image_generator;

pub struct ImageResult {
    pub bytes: Vec<u8>
}

pub trait ImageGenerator {
    async fn get_image(&self, description: String) -> Result<ImageResult>;
}

pub mod prelude {
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(target_arch = "xtensa"))]
    #[cfg(not(target_os = "android"))]
    #[cfg(feature = "openai")]
    pub use super::dalle2_image_generator::*;
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(target_os = "android"))]
    #[cfg(feature = "candle")]
    pub use super::candle_image_generator::*;
    pub use super::*;
}