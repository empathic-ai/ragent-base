use anyhow::{Result, anyhow};
use async_trait::async_trait;
use bytes::Bytes;
use futures::channel::mpsc;
use sherpa_onnx::{OnlineRecognizer, OnlineRecognizerConfig};
use std::sync::Arc;
use tokio::sync::broadcast::{self, Receiver};

use super::{Transcriber, TranscriptionResponse};
use common::prelude::CancellationToken;

pub struct SherpaTranscriber {
    recognizer: Arc<OnlineRecognizer>,
}

impl SherpaTranscriber {
    pub fn new(config: OnlineRecognizerConfig) -> Result<Self> {
        let recognizer = OnlineRecognizer::create(&config)
            .ok_or_else(|| anyhow!("sherpa-onnx could not create the online recognizer"))?;
        Ok(Self {
            recognizer: Arc::new(recognizer),
        })
    }

    pub fn from_streaming_zipformer(
        encoder: impl Into<String>,
        decoder: impl Into<String>,
        joiner: impl Into<String>,
        tokens: impl Into<String>,
        sample_rate: u32,
    ) -> Result<Self> {
        Self::from_streaming_zipformer_config(encoder, decoder, joiner, tokens, None, sample_rate)
    }

    pub fn from_streaming_zipformer_with_bpe(
        encoder: impl Into<String>,
        decoder: impl Into<String>,
        joiner: impl Into<String>,
        tokens: impl Into<String>,
        bpe_vocab: impl Into<String>,
        sample_rate: u32,
    ) -> Result<Self> {
        Self::from_streaming_zipformer_config(
            encoder,
            decoder,
            joiner,
            tokens,
            Some(bpe_vocab.into()),
            sample_rate,
        )
    }

    fn from_streaming_zipformer_config(
        encoder: impl Into<String>,
        decoder: impl Into<String>,
        joiner: impl Into<String>,
        tokens: impl Into<String>,
        bpe_vocab: Option<String>,
        sample_rate: u32,
    ) -> Result<Self> {
        let mut config = OnlineRecognizerConfig::default();
        config.feat_config.sample_rate = sample_rate as i32;
        config.model_config.transducer.encoder = Some(encoder.into());
        config.model_config.transducer.decoder = Some(decoder.into());
        config.model_config.transducer.joiner = Some(joiner.into());
        config.model_config.tokens = Some(tokens.into());
        config.model_config.bpe_vocab = bpe_vocab;
        if config.model_config.bpe_vocab.is_some() {
            config.model_config.modeling_unit = Some("bpe".into());
        }
        config.decoding_method = Some("greedy_search".into());
        config.enable_endpoint = true;
        config.rule1_min_trailing_silence = 0.8;
        config.rule2_min_trailing_silence = 1.2;
        Self::new(config)
    }
}

fn pcm16_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
        .collect()
}

#[async_trait]
impl Transcriber for SherpaTranscriber {
    async fn transcribe_stream(
        &mut self,
        sample_rate: u32,
        mut stream: Receiver<Bytes>,
        token: CancellationToken,
    ) -> Result<mpsc::UnboundedReceiver<Result<TranscriptionResponse>>> {
        let recognizer = Arc::clone(&self.recognizer);
        let (mut output, receiver) = mpsc::unbounded();

        tokio::spawn(async move {
            let input = recognizer.create_stream();
            let mut last_transcript = String::new();
            loop {
                let chunk = tokio::select! {
                    _ = wait_for_cancel(&token) => break,
                    chunk = stream.recv() => match chunk {
                        Ok(chunk) => chunk,
                        Err(broadcast::error::RecvError::Closed) => {
                            input.input_finished();
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    },
                };

                let samples = pcm16_to_f32(&chunk);
                if samples.is_empty() {
                    continue;
                }
                input.accept_waveform(sample_rate as i32, &samples);
                while recognizer.is_ready(&input) {
                    recognizer.decode(&input);
                }
                if let Some(result) = recognizer.get_result(&input) {
                    if result.text != last_transcript || recognizer.is_endpoint(&input) {
                        let is_final = recognizer.is_endpoint(&input) || result.is_final;
                        last_transcript.clone_from(&result.text);
                        if output
                            .unbounded_send(Ok(TranscriptionResponse {
                                transcript: result.text,
                                is_final,
                                speech_final: is_final,
                                usage_unit: "seconds".into(),
                                ..Default::default()
                            }))
                            .is_err()
                        {
                            break;
                        }
                        if is_final {
                            recognizer.reset(&input);
                            last_transcript.clear();
                        }
                    }
                }
            }
        });

        Ok(receiver)
    }
}

async fn wait_for_cancel(token: &CancellationToken) {
    while !token.is_cancelled() {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::pcm16_to_f32;

    #[test]
    fn converts_little_endian_pcm16() {
        let samples = pcm16_to_f32(&[0, 0, 0, 128, 255, 127]);
        assert_eq!(samples, vec![0.0, -1.0, 32767.0 / 32768.0]);
    }
}
