#![allow(warnings)]
use async_trait::async_trait;
use tokio::runtime::Handle;
use tokio::sync::{Mutex, RwLock};

use std::env;
use std::sync::Arc;
use std::thread;

use bytes::{BufMut, Bytes, BytesMut};
use crossbeam::channel::RecvError;
use futures::channel::mpsc;
use futures::stream::StreamExt;
use futures::SinkExt;

use std::error::Error;

use crate::tools::TranscriptionResponse;

use super::{result, Transcriber};
use anyhow::Result;
use tokio::sync::broadcast::{self, Sender, Receiver};

use common::prelude::*;
pub struct WebSpeechTranscriber {
    
}

impl WebSpeechTranscriber {
    pub fn new() -> Self {
        WebSpeechTranscriber {

        }
    }
}

#[async_trait]
impl Transcriber for WebSpeechTranscriber {
    async fn transcribe_stream(&mut self, sample_rate: u32, stream: Receiver<Bytes>, token: CancellationToken) -> Result<mpsc::UnboundedReceiver<Result<TranscriptionResponse>>> {
        todo!()
    }
}