//! Agent playback policy over Delune buffers; conversation state stays in the worker.

use anyhow::Result;
use common::prelude::CancellationToken;
use delune::{AudioOutput, AudioWrite};
use std::time::{Duration, Instant};

/// Preserve unwritten samples when the bounded output buffer applies backpressure.
pub(super) async fn write_speaker_samples(
    speaker_output: &mut AudioOutput,
    mut remaining: &[i16],
    token: &CancellationToken,
) -> Result<()> {
    let mut progress = Instant::now();
    while !remaining.is_empty() && !token.is_cancelled() {
        let count = speaker_output.write(remaining);
        if count > 0 {
            remaining = &remaining[count..];
            progress = Instant::now();
        } else if progress.elapsed() >= Duration::from_secs(2) {
            anyhow::bail!("Agent playback stalled with {} samples remaining", remaining.len());
        } else {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
    Ok(())
}

/// Clearing discards queued and unwritten audio, not samples already in the output buffer.
pub(super) async fn run_speaker_output_sink(
    mut output: AudioOutput,
    mut receiver: tokio::sync::mpsc::Receiver<Vec<i16>>,
    mut clear: tokio::sync::watch::Receiver<u64>,
    token: CancellationToken,
) {
    loop {
        tokio::select! {
            _ = wait_for_cancellation(token.clone()) => break,
            changed = clear.changed() => {
                if changed.is_err() {
                    break;
                }
                while receiver.try_recv().is_ok() {}
            }
            samples = receiver.recv() => {
                let Some(samples) = samples else { break; };
                tokio::select! {
                    _ = wait_for_cancellation(token.clone()) => break,
                    result = write_speaker_samples(&mut output, &samples, &token) => {
                        if let Err(error) = result {
                            tracing::warn!(%error, "Speaker output sink stalled; stopping sink worker");
                            break;
                        }
                    }
                    changed = clear.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        while receiver.try_recv().is_ok() {}
                    }
                }
            }
        }
    }
}

pub(super) async fn wait_for_cancellation(token: CancellationToken) {
    while !token.is_cancelled() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Advance one shared deadline so per-chunk scheduling delays do not accumulate.
pub(super) async fn pace_speaker_dispatch(
    sample_count: usize,
    channels: u8,
    sample_rate: u32,
    deadline: &mut Instant,
    token: &CancellationToken,
) -> bool {
    if channels == 0 || sample_rate == 0 {
        return false;
    }
    let frames = sample_count / channels as usize;
    *deadline += Duration::from_secs_f64(frames as f64 / sample_rate as f64);
    tokio::select! {
        _ = tokio::time::sleep_until((*deadline).into()) => true,
        _ = wait_for_cancellation(token.clone()) => false,
    }
}