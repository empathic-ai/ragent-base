#![allow(warnings)]
use async_trait::async_trait;
use tokio::sync::broadcast::{self, Sender, Receiver};
use bytes::{BufMut, Bytes, BytesMut};
use anyhow::Result;
use futures::channel::mpsc;
use common::prelude::*;
use super::{result, Transcriber};

pub struct WhisperTranscriber {
    
}

impl WhisperTranscriber {
    pub fn new() -> Self {
        WhisperTranscriber {

        }
    }
}

#[async_trait]
impl Transcriber for WhisperTranscriber {
    async fn transcribe_stream(&mut self, sample_rate: u32, stream: Receiver<Bytes>, token: CancellationToken) -> Result<mpsc::Receiver<Result<String>>> {
        todo!();
    }
}