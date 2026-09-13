use crate::{MAX_CLIP_SAMPLES, SAMPLE_RATE, SpeakerAudio};
use anyhow::{Result, ensure};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleSpan {
    pub start: u64,
    pub end: u64,
}
impl SampleSpan {
    pub fn new(start: u64, end: u64) -> Result<Self> {
        ensure!(start < end, "empty or inverted audio span");
        Ok(Self { start, end })
    }
    pub fn from_seconds(start: f64, end: f64) -> Result<Self> {
        ensure!(
            start.is_finite()
                && end.is_finite()
                && start >= 0.
                && end > start
                && end < u64::MAX as f64 / SAMPLE_RATE as f64,
            "invalid audio timestamp"
        );
        Self::new(
            (start * SAMPLE_RATE as f64).round() as u64,
            (end * SAMPLE_RATE as f64).round() as u64,
        )
    }
    pub fn overlap(self, other: Self) -> u64 {
        self.end
            .min(other.end)
            .saturating_sub(self.start.max(other.start))
    }
}
#[derive(Clone, Debug)]
pub struct AudioFrame {
    pub start_sample: u64,
    pub pcm16: Bytes,
}
impl AudioFrame {
    pub fn end_sample(&self) -> u64 {
        self.start_sample + self.pcm16.len() as u64 / 2
    }
}

/// History is recorded before provider fanout, never by consuming an STT receiver.
/// Only contiguous complete spans may be read. Audio is never persisted here.
pub struct AudioTimeline {
    samples: VecDeque<i16>,
    start: u64,
    end: u64,
    capacity: usize,
}
impl AudioTimeline {
    pub fn new(seconds: usize) -> Result<Self> {
        ensure!(
            (1..=120).contains(&seconds),
            "history duration must be 1–120s"
        );
        Ok(Self {
            samples: VecDeque::new(),
            start: 0,
            end: 0,
            capacity: seconds * SAMPLE_RATE as usize,
        })
    }
    pub fn end_sample(&self) -> u64 {
        self.end
    }
    pub fn push(&mut self, frame: &AudioFrame) -> Result<()> {
        ensure!(
            frame
                .start_sample
                .checked_add(frame.pcm16.len() as u64 / 2)
                .is_some(),
            "audio clock overflow"
        );
        ensure!(
            frame.pcm16.len().is_multiple_of(2),
            "PCM16 frame has an incomplete sample"
        );
        ensure!(
            frame.pcm16.len() <= MAX_CLIP_SAMPLES * 2,
            "audio frame exceeds 30s limit"
        );
        if self.end != frame.start_sample {
            self.samples.clear();
            self.start = frame.start_sample;
        }
        self.samples.extend(
            frame
                .pcm16
                .chunks_exact(2)
                .map(|v| i16::from_le_bytes([v[0], v[1]])),
        );
        self.end = frame.end_sample();
        if self.samples.len() > self.capacity {
            self.samples.drain(..self.samples.len() - self.capacity);
        }
        self.start = self.end - self.samples.len() as u64;
        Ok(())
    }
    pub fn clip(&self, span: SampleSpan) -> Result<SpeakerAudio> {
        ensure!(
            span.end > span.start && span.start >= self.start && span.end <= self.end,
            "audio missing, evicted or not yet received"
        );
        ensure!(
            span.end - span.start <= MAX_CLIP_SAMPLES as u64,
            "identity clip exceeds 30s limit"
        );
        Ok(SpeakerAudio {
            samples: self
                .samples
                .iter()
                .skip((span.start - self.start) as usize)
                .take((span.end - span.start) as usize)
                .map(|v| *v as f32 / 32768.0)
                .collect(),
        })
    }
}

/// Pick a label only when one speaker covers most of the span and no competing
/// speaker overlaps it. Silence/uncovered words stay unknown.
pub fn label_span(
    span: SampleSpan,
    segments: &[crate::DiarizationSegment],
) -> Option<crate::SpeakerLabel> {
    let matching: Vec<_> = segments
        .iter()
        .filter(|s| s.span.overlap(span) > 0)
        .collect();
    let first = matching.first()?;
    if matching.iter().any(|s| s.overlap || s.label != first.label) {
        return None;
    }
    let mut intervals: Vec<_> = matching
        .iter()
        .map(|s| (s.span.start.max(span.start), s.span.end.min(span.end)))
        .collect();
    intervals.sort_unstable();
    let mut coverage = 0;
    let mut end = span.start;
    for (s, e) in intervals {
        coverage += e.saturating_sub(s.max(end));
        end = end.max(e);
    }
    (span.end > span.start && coverage as f64 / (span.end - span.start) as f64 >= 0.8)
        .then(|| first.label.clone())
}

/// Partition a multi-speaker transcript by finalized diarization boundaries.
/// The text remains one utterance; annotations identify its audio ranges.
pub fn split_speaker_span(
    span: SampleSpan,
    segments: &[crate::DiarizationSegment],
) -> Vec<(SampleSpan, Option<crate::SpeakerLabel>)> {
    if let Some(label) = label_span(span, segments) {
        return vec![(span, Some(label))];
    }
    let mut boundaries = vec![span.start, span.end];
    for segment in segments.iter().filter(|s| s.span.overlap(span) > 0) {
        boundaries.push(segment.span.start.max(span.start));
        boundaries.push(segment.span.end.min(span.end));
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut result: Vec<(SampleSpan, Option<crate::SpeakerLabel>)> = vec![];
    for window in boundaries.windows(2) {
        let piece = SampleSpan {
            start: window[0],
            end: window[1],
        };
        let label = label_span(piece, segments);
        if let Some((previous, old_label)) = result.last_mut().filter(|(_, old)| *old == label) {
            previous.end = piece.end;
            let _ = old_label;
        } else {
            result.push((piece, label));
        }
    }
    // Bound per-utterance inference work, especially remote identify jobs.
    if result.len() > 8 {
        vec![(span, None)]
    } else {
        result
    }
}
