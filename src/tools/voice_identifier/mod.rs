// TODO: Finish implementation
//#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa")))]
//pub mod azure_identifier;
//#[cfg(not(any(target_arch = "wasm32", target_arch = "xtensa")))]
//pub use azure_identifier::*;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait VoiceIdentifier: Send + Sync {
    async fn create_voice_profile(&self, voice_name: &str) -> Result<()>;
    async fn identify_voice(&self, voice_names: Vec<String>) -> Result<VoiceIdentificationResponse>;
}

#[derive(Clone)]
pub struct VoiceIdentificationResponse {
    pub voice_name: String
}