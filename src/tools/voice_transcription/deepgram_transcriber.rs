#![allow(warnings)]
use async_trait::async_trait;
use deepgram;
use tokio::runtime::Handle;
use tokio::sync::{Mutex, RwLock};

use std::env;
use std::sync::Arc;
use std::thread;

use bytes::{BufMut, Bytes, BytesMut};
//use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
//use cpal::Sample;
use crossbeam::channel::RecvError;
use futures::channel::mpsc;
use futures::stream::StreamExt;
use futures::SinkExt;

use deepgram::{Deepgram, DeepgramError};
use deepgram::transcription::live::StreamResponse::{TranscriptResponse, TerminalResponse};
use std::error::Error;

use crate::tools::TranscriptionResponse;

use super::{result, Transcriber};
use anyhow::Result;
use tokio::sync::broadcast::{self, Sender, Receiver};

use common::prelude::*;
pub struct DeepgramTranscriber {
    languages: Vec<String>
}

impl DeepgramTranscriber {
    pub fn new_from_env() -> Self {
        DeepgramTranscriber {
            languages: vec!["en".to_string()]
        }
    }
}

#[async_trait]
impl Transcriber for DeepgramTranscriber {
        // TODO: Re-connect after connection closes due to timeout
    // Possibly use VAD for detecting when to re-initialize a connection?
    // 48000
    async fn transcribe_stream(&mut self, sample_rate: u32, stream: Receiver<Bytes>, token: CancellationToken) -> Result<mpsc::UnboundedReceiver<Result<TranscriptionResponse>>> {

        // If only one language is supported, use Deepgram's streaming mode (which don't support language detection)
        // Otherwise, use Deepgram's non-streaming mode
        if self.languages.len() == 1 {
            let (mut async_tx, async_rx) = mpsc::unbounded::<Result<TranscriptionResponse>>();

            let stream = Arc::new(Mutex::new(stream));

            //println!("Getting Deepgram stream...");
            let _token = token.clone();
            
            tokio::task::spawn(async move {
                loop {
                    let _token = _token.clone();
                    if _token.is_cancelled() {
                        continue;
                    }

                    let stream_clone = stream.clone();

                    let item = stream.lock().await.recv().await.clone();
                    
                    if let Ok(item) = item {

                        //println!("GOT VOICE DATA ITEM!");
                        let (mut forward_tx, mut forward_rx) = mpsc::channel::<Result<Bytes>>(16);

                        let is_terminated = Arc::new(Mutex::new(false));
        
                        let _is_terminated = is_terminated.clone();

                        let mut _forward_tx = forward_tx.clone();

                        // Start a task to forward items from the original stream to the new stream
                        tokio::spawn(async move {
                            let mut locked_stream = stream_clone.lock().await;

                            while let Ok(item) = locked_stream.recv().await {
                                let _token = _token.clone();
                                if _token.is_cancelled() {
                                    continue;
                                }
        
                                if _is_terminated.lock().await.to_owned() {
                                    break;
                                }
                                if forward_tx.send(Ok(item)).await.is_err() {
                                    //panic!("STREAM ERROR");
                                    break;
                                }
                            }
                        });
        
                        let dg = Deepgram::new(env::var("DEEPGRAM_API_KEY").unwrap());
        
                        //println!("Getting Deepgram results...");
                        
                        let mut results = dg
                            .transcription()
                            .stream_request()
                            .stream(forward_rx)
                            // TODO Enum.
                            .encoding("linear16".to_string())
                            // TODO Specific to my machine, not general enough example.
                            .sample_rate(sample_rate)//44100)
                            // TODO Specific to my machine, not general enough example.
                            .channels(2)
                            .start()
                            .await;
                        
                        println!("Sending first voice item!");
       
                        if let Err(err) = _forward_tx.send(Ok(item)).await {
                            println!("Error sending initial voice data to transcriber! Wiil try restarting: {}", err);
                            continue;
                        }
                        println!("Sent first voice item!");

                        match results {
                            Ok(mut results) => {
                                let _token = token.clone();
                                while let Some(result) = results.next().await {
                                    let _token = _token.clone();
            
                                    if _token.is_cancelled() {
                                        continue;
                                    }
            
                                    match result {
                                        Ok(result) => {
                                            match result {
                                                TranscriptResponse { duration, is_final, channel } => {
                                                    let mut transcript_responses = Vec::<TranscriptionResponse>::new();
    
                                                    let first_alternative = &channel.alternatives.first().unwrap();
                                                    for word in first_alternative.words.iter() {
                                                        if word.confidence < 0.5 {
                                                            transcript_responses.push(TranscriptionResponse { speaker: None, transcript: word.word.clone() });
                                                        } else {
                                                            if let Some(mut last) = transcript_responses.last_mut() {
                                                                if word.speaker == last.speaker {
                                                                    last.transcript += &(" ".to_string() + word.word.as_str());
                                                                    continue;
                                                                }
                                                            }
                                                            transcript_responses.push(TranscriptionResponse { speaker: word.speaker.clone(), transcript: word.word.clone() });
                                                        }
                                                    }
                                                    
                                                    //let transcript = first_alternative.transcript;
                                                    //println!("Transcript: {:?}", transcript);
    
                                                    for response in transcript_responses {
                                                        //println!("[Speaker:{:?}] {:?}", response.speaker.unwrap(), response.transcript);
                                                        async_tx.send(Ok(response.clone())).await;
                                                    }
                                                },
                                                TerminalResponse { request_id, created, duration, channels } => {
                                                    *is_terminated.lock().await = true;
                                                    // Connection closed--will need to reconnect
                                                    //async_tx.close().await;
                                                    //break;
                                                    println!("Deepgram terminated");
                                                    break;
                                                },
                                            }
                                        },
                                        Err(err) => {
                                            println!("DEEPGRAM ERROR: {}", err.to_string())
                                        },
                                    }
                                }
                            },
                            Err(err) => {
                                println!("Failed to get Deepgram transcription: {}", err);
                                break;
                            }
                        }
                    }

                }
            });

            Ok(async_rx)
        } else {
            
            todo!();
            /*
            let dg = Deepgram::new(env::var("DEEPGRAM_API_KEY").unwrap());

            let receiver = delune::volume_vad_filter(stream);
            while let Some(item) = receiver.recv().await {
                dg.transcription().prerecorded(deepgram::transcription::prerecorded::audio_source::AudioSource::from_buffer(item), deepgram::transcription::prerecorded::options::OptionsBuilder::new().detect_language(true).language(language))
            }
             */

        }
    }
}

/* 
fn microphone_as_stream() -> Receiver<Result<Bytes, RecvError>> {
    let (sync_tx, sync_rx) = crossbeam::channel::unbounded();
    let (mut async_tx, async_rx) = mpsc::channel(1);

    thread::spawn(move || {
        let host = cpal::default_host();
        let device = host.default_input_device().unwrap();

        // let config = device.supported_input_configs().unwrap();
        // for config in config {
        //     dbg!(&config);
        // }

        let config = device.default_input_config().unwrap();

        // dbg!(&config);

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device
                .build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &_| {
                        let mut bytes = BytesMut::with_capacity(data.len() * 2);
                        for sample in data {
                            bytes.put_i16_le(*sample as i16);
                        }
                        sync_tx.send(bytes.freeze()).unwrap();
                    },
                    |_| panic!(),
                    None
                )
                .unwrap(),
            cpal::SampleFormat::I16 => device
                .build_input_stream(
                    &config.into(),
                    move |data: &[i16], _: &_| {
                        let mut bytes = BytesMut::with_capacity(data.len() * 2);
                        for sample in data {
                            bytes.put_i16_le(*sample);
                        }
                        sync_tx.send(bytes.freeze()).unwrap();
                    },
                    |_| panic!(),
                    None
                )
                .unwrap(),
            cpal::SampleFormat::U16 => device
                .build_input_stream(
                    &config.into(),
                    move |data: &[u16], _: &_| {
                        let mut bytes = BytesMut::with_capacity(data.len() * 2);
                        for sample in data {
                            bytes.put_i16_le(*sample as i16);
                        }
                        sync_tx.send(bytes.freeze()).unwrap();
                    },
                    |_| panic!(),
                    None
                )
                .unwrap(),
            _ => todo!()
        };

        stream.play().unwrap();

        loop {
            thread::park();
        }
    });

    tokio::spawn(async move {
        loop {
            let data = sync_rx.recv();
            async_tx.send(data).await.unwrap();
        }
    });

    async_rx
}
*/

/*
async fn main() -> Result<(), DeepgramError> {
    let dg = Deepgram::new(env::var("DEEPGRAM_API_KEY").unwrap());

    let mut results = dg
        .transcription()
        .stream_request()
        .stream(microphone_as_stream())
        // TODO Enum.
        .encoding("linear16".to_string())
        // TODO Specific to my machine, not general enough example.
        .sample_rate(44100)
        // TODO Specific to my machine, not general enough example.
        .channels(2)
        .start()
        .await?;

    while let Some(result) = results.next().await {
        println!("got: {:?}", result);
    }

    Ok(())
}
 */