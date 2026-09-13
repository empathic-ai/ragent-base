#![allow(warnings)]
use async_trait::async_trait;
use deepgram;
use deepgram::common::options::Model;

#[path = "deepgram_stream_config.rs"]
mod stream_config;
use tokio::runtime::Handle;
use tokio::sync::{Mutex, RwLock};

use std::env;
use std::sync::Arc;
use std::thread;

use bytes::{BufMut, Bytes, BytesMut};
//use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
//use cpal::Sample;
use crossbeam::channel::RecvError;
use futures::SinkExt;
use futures::channel::mpsc;
use futures_lite::stream::{Boxed, StreamExt as LiteStreamExt};

use deepgram::common::flux_response::{FluxResponse, TurnEvent};
use deepgram::common::stream_response::StreamResponse;
use deepgram::{Deepgram, DeepgramError};
use deepgram::common::options::Encoding;
use std::error::Error;

use crate::tools::TranscriptionResponse;

use super::{Transcriber, result};
use anyhow::{anyhow, Context, Result};
use tokio::sync::broadcast::{self, Receiver, Sender};

use bevy::prelude::*;
use common::prelude::*;

pub struct DeepgramTranscriber {
    languages: Vec<String>,
    diarize: bool,
}

enum DeepgramEvent {
    Flux(FluxResponse),
    Standard(StreamResponse),
}

fn is_flux_model(model: &Model) -> bool {
    match model {
        Model::FluxGeneralEn => true,
        _ => false,
    }
}

// The shared project's token is atomic-only, without a notification future.
async fn cancelled(token: &CancellationToken) {
    while !token.is_cancelled() {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

impl DeepgramTranscriber {
    pub fn with_diarization(mut self, enabled: bool) -> Self { self.diarize = enabled; self }
    pub fn new_from_env() -> Self {
        DeepgramTranscriber {
            languages: vec!["en".to_string()],
            diarize: true,
        }
    }
}

#[async_trait]
impl Transcriber for DeepgramTranscriber {
    async fn transcribe_stream(
        &mut self,
        sample_rate: u32,
        stream: Receiver<Bytes>,
        token: CancellationToken,
    ) -> Result<mpsc::UnboundedReceiver<Result<TranscriptionResponse>>> {
        let diarize = self.diarize;
        // If only one language is supported, use Deepgram's streaming mode (which don't support language detection)
        // Otherwise, use Deepgram's non-streaming mode
        if self.languages.len() == 1 {
            let api_key = env::var("DEEPGRAM_API_KEY").context("Missing DEEPGRAM_API_KEY")?;
            let (mut async_tx, async_rx) = mpsc::unbounded::<Result<TranscriptionResponse>>();

            let stream = Arc::new(Mutex::new(stream));
            let clock = Arc::new(std::sync::atomic::AtomicU64::new(0));
            let clock_valid = Arc::new(std::sync::atomic::AtomicBool::new(true));

            //println!("Getting Deepgram stream...");
            let _token = token.clone();

            tokio::task::spawn(async move {
                loop {
                    let _token = _token.clone();
                    if _token.is_cancelled() {
                        break;
                    }

                    let stream_clone = stream.clone();

                    let item = tokio::select! {
                        _ = cancelled(&token) => break,
                        item = async { stream.lock().await.recv().await } => item,
                    };
                    if matches!(item, Err(broadcast::error::RecvError::Closed)) { break; }
                    if let Err(broadcast::error::RecvError::Lagged(n)) = item {
                        clock_valid.store(false, std::sync::atomic::Ordering::SeqCst);
                        warn!("Transcription stream lagged while reconnecting, dropped {n} chunks");
                        continue;
                    }

                    if let Ok(item) = item {
                        let session_id = uuid::Uuid::new_v4().to_string();
                        let stream_start = clock.fetch_add(item.len() as u64 / 2, std::sync::atomic::Ordering::SeqCst);
                        if item.len() % 2 != 0 { clock_valid.store(false, std::sync::atomic::Ordering::SeqCst); }
                        let forward_clock = clock.clone();
                        let forward_valid = clock_valid.clone();
                        //println!("GOT VOICE DATA ITEM!");
                        let (mut forward_tx, mut forward_rx) = mpsc::channel::<std::result::Result<Bytes, deepgram::DeepgramError>>(16);

                        let (forward_done_tx, mut forward_done_rx) = tokio::sync::oneshot::channel();

                        // Queue the first item before later audio can overtake it.
                        if forward_tx.send(Ok(item)).await.is_err() {
                            break;
                        }

                        // Start a task to forward items from the original stream to the new stream
                        let forward_task = tokio::spawn(async move {
                            let mut locked_stream = stream_clone.lock().await;
                            let mut forwarded_bytes = 0usize;
                            let mut reported_at = std::time::Instant::now();

                            loop {
                                let item = tokio::select! {
                                    _ = cancelled(&_token) => break,
                                    item = tokio::time::timeout(std::time::Duration::from_secs(15), locked_stream.recv()) => {
                                        match item {
                                            Ok(item) => item,
                                            Err(_) => {
                                                info!("Deepgram input idle for 15s; closing session");
                                                break;
                                            }
                                        }
                                    },
                                };
                                match item {
                                    Ok(item) => {
                                        let _token = _token.clone();
                                        if _token.is_cancelled() {
                                            break;
                                        }

                                        forward_clock.fetch_add(item.len() as u64 / 2, std::sync::atomic::Ordering::SeqCst);
                                        if item.len() % 2 != 0 { forward_valid.store(false, std::sync::atomic::Ordering::SeqCst); }
                                        forwarded_bytes += item.len();
                                        let sent = tokio::select! {
                                            _ = cancelled(&_token) => break,
                                            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                                                warn!("Deepgram audio forwarding stalled for 10s; closing session");
                                                break;
                                            },
                                            sent = forward_tx.send(Ok(item)) => sent,
                                        };
                                        if sent.is_err() {
                                            //panic!("STREAM ERROR");
                                            break;
                                        }
                                        if reported_at.elapsed() >= std::time::Duration::from_secs(5) {
                                            info!("Deepgram input: {} bytes ({} ms audio) forwarded in {:?}; {} messages pending",
                                                forwarded_bytes, forwarded_bytes * 1000 / (sample_rate as usize * 2),
                                                reported_at.elapsed(), locked_stream.len());
                                            forwarded_bytes = 0;
                                            reported_at = std::time::Instant::now();
                                        }
                                    }
                                    Err(broadcast::error::RecvError::Lagged(n)) => {
                                        forward_valid.store(false, std::sync::atomic::Ordering::SeqCst);
                                        warn!("Transcription stream lagged, dropped {n} chunks — continuing");
                                        continue;
                                    }
                                    Err(broadcast::error::RecvError::Closed) => break,
                                }
                            }
                            let _ = forward_done_tx.send(());
                        });

                        let dg = Deepgram::new(api_key.clone()).expect("Failed to start Deepgram API");

                        let model = Model::Nova3;
                        let use_flux = is_flux_model(&model);
                        let standard_model = model.clone();
                        info!("Connecting to Deepgram using {}...", if use_flux { "FluxGeneralEn" } else { "standard model" });

                        let connect_started = std::time::Instant::now();
                        let connect = async move {
                            let transcription = dg.transcription();
                            if use_flux {
                                let results = transcription
                                    .flux_request()
                                    .encoding(Encoding::Linear16)
                                    .sample_rate(sample_rate)
                                    .stream(forward_rx)
                                    .await?;
                                Ok::<Boxed<std::result::Result<DeepgramEvent, DeepgramError>>, DeepgramError>(
                                    Box::pin(LiteStreamExt::map(results, |result| result.map(DeepgramEvent::Flux))),
                                )
                            } else {
                                let results = stream_config::standard_request(&dg, standard_model, sample_rate, diarize)
                                    .stream(forward_rx)
                                    .await?;
                                Ok::<Boxed<std::result::Result<DeepgramEvent, DeepgramError>>, DeepgramError>(
                                    Box::pin(LiteStreamExt::map(results, |result| result.map(DeepgramEvent::Standard))),
                                )
                            }
                        };

                        let results = tokio::select! {
                            _ = cancelled(&token) => {
                                forward_task.abort();
                                let _ = forward_task.await;
                                break;
                            },
                            results = tokio::time::timeout(std::time::Duration::from_secs(10), connect) => {
                                match results {
                                    Ok(results) => results,
                                    Err(err) => Err(DeepgramError::InternalClientError(anyhow!("Deepgram connection timed out: {err}"))),
                                }
                            },
                        };

                        match results {
                            Ok(mut results) => {
                                info!("Deepgram connected in {:?} (PCM16 mono, {} Hz)", connect_started.elapsed(), sample_rate);
                                let _token = token.clone();
                                let mut drain_deadline = None;
                                loop {
                                    let result = tokio::select! {
                                        _ = cancelled(&token) => break,
                                        _ = &mut forward_done_rx, if drain_deadline.is_none() => {
                                            // Let the SDK finalize buffered audio, but never
                                            // wait forever for a terminal response on a dead socket.
                                            drain_deadline = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(2));
                                            continue;
                                        },
                                        _ = async {
                                            match drain_deadline {
                                                Some(deadline) => tokio::time::sleep_until(deadline).await,
                                                None => std::future::pending::<()>().await,
                                            }
                                        } => {
                                            warn!("Deepgram did not finish within 2s of input closing; releasing session");
                                            break;
                                        },
                                        result = LiteStreamExt::next(&mut results) => result,
                                    };
                                    let Some(result) = result else { break; };
                                    let _token = _token.clone();

                                    if _token.is_cancelled() {
                                        break;
                                    }

                                    match result {
                                        Ok(DeepgramEvent::Flux(response)) => {
                                            match response {
                                                FluxResponse::TurnInfo {
                                                    event: TurnEvent::EndOfTurn,
                                                    transcript,
                                                    ..
                                                } => {
                                                    if !transcript.trim().is_empty() {
                                                        async_tx.send(Ok(TranscriptionResponse {
                                                            speaker: None,
                                                            transcript,
                                                            is_final: true,
                                                            speech_final: true,
                                                            ..Default::default()
                                                        })).await;
                                                    }
                                                }
                                                FluxResponse::TurnInfo { .. } => {}
                                                FluxResponse::Connected { request_id, .. } => {
                                                    info!("Deepgram Flux session ready (ID: {request_id})");
                                                }
                                                FluxResponse::FatalError { code, description, .. } => {
                                                    warn!("Deepgram Flux fatal error {code}: {description}");
                                                    break;
                                                }
                                                FluxResponse::ConfigureSuccess { .. }
                                                | FluxResponse::ConfigureFailure { .. }
                                                | FluxResponse::Unknown(_) => {},
                                                _ => {}
                                            }
                                        }
                                        Ok(DeepgramEvent::Standard(response)) => {
                                            match response {
                                                StreamResponse::TranscriptResponse {
                                                    is_final,
                                                    speech_final,
                                                    start,
                                                    duration,
                                                    channel,
                                                    ..
                                                } => {
                                                    let Some(alternative) = channel.alternatives.first() else {
                                                        continue;
                                                    };
                                                    if !alternative.transcript.trim().is_empty() {
                                                        info!("Deepgram result: is_final={is_final}, speech_final={speech_final}, audio_end={:.3}s, session_elapsed={:?}",
                                                            start + duration, connect_started.elapsed());
                                                        if !is_final { continue; }
                                                        let grouped = group_words_by_speaker(&alternative.words);
                                                        if grouped.is_empty() {
                                                            async_tx.send(Ok(TranscriptionResponse {
                                                                session_id: Some(session_id.clone()),
                                                                stream_start_sample: clock_valid.load(std::sync::atomic::Ordering::SeqCst).then_some(stream_start),
                                                                transcript: alternative.transcript.clone(),
                                                                start_seconds: Some(start),
                                                                end_seconds: Some(start + duration),
                                                                is_final,
                                                                speech_final,
                                                                ..Default::default()
                                                            })).await;
                                                        } else {
                                                            let group_count = grouped.len();
                                                            for (index, group) in grouped.into_iter().enumerate() {
                                                                async_tx.send(Ok(TranscriptionResponse {
                                                                    session_id: Some(session_id.clone()),
                                                                    stream_start_sample: clock_valid.load(std::sync::atomic::Ordering::SeqCst).then_some(stream_start),
                                                                    speaker: group.speaker,
                                                                    diarization_label: group.speaker.map(|speaker| speaker.to_string()),
                                                                    transcript: group.text,
                                                                    start_seconds: Some(group.start),
                                                                    end_seconds: Some(group.end),
                                                                    is_final,
                                                                    speech_final: speech_final && index + 1 == group_count,
                                                                    ..Default::default()
                                                                })).await;
                                                            }
                                                        }
                                                    }
                                                }
                                                StreamResponse::TerminalResponse { request_id, .. } => {
                                                    info!("Deepgram session completed (ID: {request_id})");
                                                    break;
                                                }
                                                _ => {}
                                            }
                                        }
                                        Err(err) => {
                                            warn!("Deepgram stream failed; reconnecting: {err}");
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("Failed to get Deepgram transcription: {}", err);
                                forward_task.abort();
                                let _ = forward_task.await;
                                tokio::select! {
                                    _ = cancelled(&token) => break,
                                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {},
                                }
                                continue;
                            }
                        }
                        // Release the shared receiver even on EOF/error, not
                        // only when a terminal metadata message was received.
                        forward_task.abort();
                        let _ = forward_task.await;
                        info!("Deepgram stream ended; waiting for audio to reconnect");
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

struct SpeakerWordGroup {
    speaker: Option<i32>,
    text: String,
    start: f64,
    end: f64,
}

fn group_words_by_speaker(words: &[deepgram::common::stream_response::Word]) -> Vec<SpeakerWordGroup> {
    let mut groups = Vec::<SpeakerWordGroup>::new();
    for word in words {
        let text = word.punctuated_word.as_deref().unwrap_or(&word.word);
        if let Some(group) = groups.last_mut().filter(|group| group.speaker == word.speaker) {
            group.text.push(' ');
            group.text.push_str(text);
            group.end = word.end;
        } else {
            groups.push(SpeakerWordGroup {
                speaker: word.speaker,
                text: text.to_string(),
                start: word.start,
                end: word.end,
            });
        }
    }
    groups
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

    while let Some(result) = futures::StreamExt::next(&mut results).await {
        println!("got: {:?}", result);
    }

    Ok(())
}
 */
