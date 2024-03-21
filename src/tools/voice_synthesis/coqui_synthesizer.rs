use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::*;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoquiSynthesizer {
}

impl CoquiSynthesizer {
    pub fn new() -> Self {
        Self { }
    }
}

#[async_trait]
impl Synthesizer for CoquiSynthesizer {
    async fn create_speech(&self, emotion: String, voice_name: String, text: String) -> Result<SynthesisResult> {
        todo!();
    }
}