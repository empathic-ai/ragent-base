use crate::tools::voice_transcription::Transcriber;
use uuid::Uuid;

pub struct UserAgent {
    pub transcriber: Option<Box<dyn Transcriber>>,
    pub user_id: Uuid
}

impl UserAgent {
    pub fn new() -> Self {
        Self {
            transcriber: None,
            user_id: Uuid::new_v4()
        }
    }
}