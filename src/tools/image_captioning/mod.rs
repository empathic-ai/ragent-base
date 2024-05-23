#![allow(warnings)]
use bytes::Bytes;
use anyhow::Result;
use std::collections::HashMap;

pub mod candle_image_captioner;

pub struct ImageCaption {
    pub caption: String
}

pub trait ImageCaptioner {
    async fn caption_image(&self, description: String) -> Result<ImageCaption>;
}

pub mod prelude {
    pub use super::candle_image_captioner::*;
    pub use super::*;
}