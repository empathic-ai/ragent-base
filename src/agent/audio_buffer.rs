use cpal::{FromSample, Sample};
use std::collections::VecDeque;

pub struct AudioBuffer<T> {
    buffer: VecDeque<T>,
    channels: usize,
}

impl<T> AudioBuffer<T>
where
    T: Sample + FromSample<f32> + Copy,
{
    pub fn new(channels: usize) -> Self {
        AudioBuffer {
            buffer: VecDeque::new(),
            channels,
        }
    }

    pub fn write_data(&mut self, data: Vec<T>) {
        self.buffer.extend(data);
    }

    pub fn fill_output(&mut self, output: &mut [T]) {
        let output_frames = output.len() / self.channels;
        let available_frames = self.buffer.len() / self.channels;
        let frames_to_copy = std::cmp::min(output_frames, available_frames);

        for i in 0..frames_to_copy {
            for ch in 0..self.channels {
                output[i * self.channels + ch] = self.buffer.pop_front().unwrap_or_else(|| T::from_sample(0.0));
            }
        }

        // Zero-pad the rest of the output buffer if needed
        if output_frames > frames_to_copy {
            for i in frames_to_copy..output_frames {
                for ch in 0..self.channels {
                    output[i * self.channels + ch] = T::from_sample(0.0);
                }
            }
        }
    }
}
