#![allow(warnings)]
use bytes::Bytes;
use anyhow::Result;
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
pub mod dalle2;

pub struct ImageResult {
    pub bytes: Vec<u8>
}

pub trait ImageGenerator {
    async fn get_image(&self, description: String) -> Result<ImageResult>;
}

pub mod prelude {
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(not(target_arch = "xtensa"))]
    pub use super::dalle2::*;
    pub use super::*;
}