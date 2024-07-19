use std::sync::Arc;
use std::sync::Mutex;

use crate::UserEventType;

use super::UserEventWorker;
use super::UserEvent;
use bevy_builder::database::Thing;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use cpal::FromSample;
use cpal::Sample;
use cpal::SampleFormat;
use tokio::sync::broadcast::{Sender, Receiver};
use bevy::prelude::*;
use anyhow::Result;
use anyhow::anyhow;
use cpal::StreamConfig;

use super::audio_buffer::AudioBuffer;

#[cfg_attr(feature = "bevy", derive(Component))]
pub struct SpeakerWorker {
	pub space_id: Thing,
    pub input_tx: Sender<UserEvent>,
}

impl SpeakerWorker {
	pub fn new(space_id: Thing) -> Self {
		let host = cpal::default_host();
		let device = host.default_output_device().unwrap();

		// Get supported output formats
		//let mut supported_configs = device.supported_input_configs()
		//.expect("Error querying supported formats");

		println!("Speaker device name: {}", device.name().unwrap());

		let config = device.default_output_config().unwrap();
		let sample_format = config.sample_format();
		let bits_per_sample = sample_format.sample_size() * 8;

		let config = config.config();
		/*
		let format = supported_configs.find(|f| f.sample_format() == SampleFormat::I16).expect("No suitable format found");
		let config = StreamConfig {
			channels: 2,
			sample_rate: format.min_sample_rate(),
			buffer_size: cpal::BufferSize::Default,
		};
		*/
	
		let channels = config.channels as usize;
		
		let sample_rate = config.sample_rate.0;
		println!("Speaker channel count: {}", config.channels);
		println!("Speaker sample rate: {}", config.sample_rate.0);
		println!("Speaker sample format: {}", sample_format);
		//println!("Sample size: {}", sample_size);

		let (tx, mut rx) = tokio::sync::broadcast::channel::<UserEvent>(32);

		std::thread::spawn(move || {
			let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

			let stream = match sample_format {
				cpal::SampleFormat::I8 => {
					todo!()
				},
				cpal::SampleFormat::I16 => {
					device.build_output_stream(
						&config,
						move |output: &mut [u8], _: &cpal::OutputCallbackInfo| {
							if let Some(data) = try_recv_data(&mut rx) {
								let data = empathic_audio::resample_pcm(data.to_vec(), 16000, sample_rate, 2, 2, 16, 16).unwrap();
								write_data(output, 2, data);
							}
						},
						err_fn,
						None,
					).unwrap()
				},
				// cpal::SampleFormat::I24 => run::<I24>(&device, &config.into()),
				cpal::SampleFormat::I32 => {
					todo!()
				},
				// cpal::SampleFormat::I48 => run::<I48>(&device, &config.into()),
				cpal::SampleFormat::I64 => {
					todo!()
				},
				cpal::SampleFormat::U8 => {
					todo!()
				},
				cpal::SampleFormat::U16 => {
					todo!()
				},
				// cpal::SampleFormat::U24 => run::<U24>(&device, &config.into()),
				cpal::SampleFormat::U32 => {
					todo!()
				},
				// cpal::SampleFormat::U48 => run::<U48>(&device, &config.into()),
				cpal::SampleFormat::U64 => {
					todo!()
				},
				cpal::SampleFormat::F32 => {
						
					let mut buffer = Arc::new(Mutex::new(AudioBuffer::new(channels)));

					let _buffer = buffer.clone();
					std::thread::spawn(move || {
						loop {
							if let Some(data) = try_recv_data(&mut rx) {
								
								//println!("Writing data to speaker");
								let data = empathic_audio::resample_pcm(data.to_vec(), 16000, sample_rate, 2, 2, 32, 32).unwrap();
								let data = empathic_audio::convert_u8_to_f32(&data, 2, 32).unwrap();
						
								_buffer.lock().unwrap().write_data(data);
		//let data = empathic_audio::combine_channels(data);
								//write_data(output, 2, data);
							}
						}
					});

					device.build_output_stream(
						&config,
						move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
							buffer.lock().unwrap().fill_output(output);
						},
						err_fn,
						None,
					).unwrap()
				},
				cpal::SampleFormat::F64 => {
					todo!()
				},
				sample_format => panic!("Unsupported sample format '{sample_format}'"),
			};

			stream.play().unwrap();

			loop {
				std::thread::sleep(std::time::Duration::from_millis(1000));
			}
		});

		Self {
			space_id: space_id,
			input_tx: tx,
		}
	}
}

fn try_recv_data(rx: &mut Receiver<UserEvent>) -> Option<Vec<u8>> {
	if let Ok(UserEvent { user_id: _, space_id: _, context_id: _, user_event_type: Some(UserEventType::SpeakBytesEvent(ev)) }) = rx.try_recv() {
		Some(ev.data)
	} else {
		None
	}
}

fn write_data<T>(output: &mut [T], channels: usize, data: Vec<T>)
where
    T: Sample + FromSample<f32> + Copy,
{
    let data_len = data.len();
    let output_len = output.len();
    let min_len = std::cmp::min(data_len, output_len);

    // Copy data into output buffer
    for (frame, value) in output.chunks_mut(channels).zip(data.iter()).take(min_len / channels) {
        for sample in frame.iter_mut() {
            *sample = *value;
        }
    }

    // Zero-pad the rest of the output buffer if data is shorter
    if output_len > data_len {
        for frame in output.chunks_mut(channels).skip(data_len / channels) {
            for sample in frame.iter_mut() {
                *sample = T::from_sample(0.0);
            }
        }
    }
}
/*
fn write_data<T>(output: &mut [T], channels: usize, data: Vec<T>)
where
    T: Sample + FromSample<f32> + Copy,
{
    assert!(output.len() == data.len(), "Output buffer and data length must match");

    for (frame, value) in output.chunks_mut(channels).zip(data.iter()) {
        for sample in frame.iter_mut() {
            *sample = *value;
        }
    }
}
	 */

/*
fn write_data(mut output: Vec<u8>, sample_rate: u32, channels: usize, rx: &mut Receiver<UserEvent>)
{
	
	let data = ev.data;
	
	//let data = empathic_audio::separate_channels(data, bit_depth);
	for frame in output.chunks_mut(channels) {
		let mut iter = frame.iter_mut();
		for value in data.iter()  {
			if let Some(mut next) = iter.next() {
				*next = value.clone();
			}
		}
	}
}
	 */

impl UserEventWorker for SpeakerWorker {
	fn is_valid_space(&self, space_id: &Thing) -> Result<bool> {
		Ok(self.space_id == space_id.clone())
	}

	fn send_event(&mut self, ev: UserEvent) -> anyhow::Result<()> {
		//println!("Speaker worker got event!");
		self.input_tx.send(ev)?;
		Ok(())
	}

	fn try_recv_event(&mut self) -> anyhow::Result<UserEvent> {
		Err(anyhow!("No events are receivable from speaker worker."))
	}
}


