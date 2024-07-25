use anyhow::Result;
use async_trait::async_trait;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
#[cfg(not(target_os = "android"))]
pub mod eleven_labs_converter;

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "xtensa"))]
#[cfg(not(target_os = "android"))]
pub use eleven_labs_converter::*;

#[async_trait]
pub trait VoiceConverter: Send + Sync {
    async fn convert_voice(&self, voice_name: String, bytes: Vec<u8>) -> Result<VoiceConversionResult>;
}
pub struct VoiceConversionResult {
    pub bytes: Vec<u8>
}