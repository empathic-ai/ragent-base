//use std::f128::consts::E;
use std::sync::Arc;
use std::sync::Mutex;

use crate::UserEventType;

use super::UserEventWorker;
use super::UserEvent;
use bytes::Bytes;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use cpal::BufferSize;
use cpal::FromSample;
use cpal::Sample;
use cpal::SampleFormat;
use cpal::SupportedBufferSize;
use observer::Observer;
use ringbuf::storage::Heap;
use ringbuf::wrap::caching::Caching;
//use tokio::sync::broadcast::{Sender, Receiver};
use bevy::prelude::*;
use flux::prelude::*;
use anyhow::Result;
use anyhow::anyhow;
use cpal::StreamConfig;
use ringbuf::{traits::*, HeapRb};
use ringbuf::SharedRb;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;

use super::audio_buffer::AudioBuffer;

#[cfg_attr(feature = "bevy", derive(Component))]
pub struct SpeakerWorker {
	pub space_id: Thing,
	pub user_id: Thing,
	//pub buffer: Caching<Arc<SharedRb<Heap<f32>>>, true, false>,
	pub sample_rate: u32,
    pub input_tx: Sender<Bytes>,
}

impl SpeakerWorker {
	pub fn new(space_id: Thing, user_id: Thing, device_name: Option<String>) -> Self {
		let host = cpal::default_host();

		let device = {
			if let Some(device_name) = device_name {
				for device in host.output_devices().unwrap().into_iter() {
					println!("Speaker device: {}", device.name().unwrap());
				}
				let device = host.output_devices().unwrap().find(|x| x.name().unwrap() == device_name).unwrap();
				device
			} else {
				host.default_output_device().unwrap()
			}
		};

		//let device = host.default_output_device().unwrap();

		// Get supported output formats
		//let mut supported_configs = device.supported_input_configs()
		//.expect("Error querying supported formats");

		println!("Speaker device name: {}", device.name().unwrap());

		let config = device.default_output_config().unwrap();
		let sample_format = config.sample_format();
		let sample_size = sample_format.sample_size();
		let bits_per_sample = sample_size * 8;

		if let SupportedBufferSize::Range { min, max } = config.buffer_size() {
			println!("Speaker min frames: {} Speaker max frames {}", min, max);
		}

		let mut config = config.config();
		config.buffer_size = BufferSize::Fixed(1);

		let latency_frames = (100.0 / 1_000.0) * config.sample_rate.0 as f32;
		let latency_samples = latency_frames as usize * config.channels as usize;
	
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

		let (tx, mut rx) = tokio::sync::mpsc::channel::<Bytes>(32);
		//let mut buffer = Arc::new(Mutex::new(AudioBuffer::new(channels)));

		//let _buffer = buffer.clone();

		let _user_id = user_id.clone();
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
							/*
							if let Some(data) = try_recv_data(&mut rx) {
								let data = delune::resample_pcm(data.to_vec(), 16000, sample_rate, 2, 2, 16, 16).unwrap();
								write_data(output, 2, data);
							}
							 */
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

					// Previously 2
					let rb = HeapRb::<f32>::new(latency_samples * 100);
					let (mut prod, mut cons) = rb.split();

					//for _ in 0..latency_samples {
						// The ring buffer has twice as much space as necessary to add latency here,
						// so this should never fail
					//	prod.try_push(0.0).unwrap();
					//}

					std::thread::spawn(move || {
						loop {
							if let Some(data) = try_recv_data(_user_id.clone(), &mut rx) {
								
								let data = delune::resample_pcm(data.to_vec(), 16000, sample_rate, 1, channels as u32, 16, 16).unwrap();
								let data = delune::convert_u8_to_f32(&data, channels as u32, 16).unwrap();
			
								prod.push_slice(&data);

								/*
								for e in data {
									if !prod.try_push(e).is_ok() {
										println!("Failed to push! Skipping.");
										break;
									}
								}
								 */
								
								//_buffer.lock().unwrap().write_data(data);
								//let data = delune::combine_channels(data);
								//write_data(output, 2, data);
							}
							//std::thread::sleep(std::time::Duration::from_millis(10));
						}
					});

					device.build_output_stream(
						&config,
						move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {

							/*		
							if cons.occupied_len() > sample_size*100000 {
								println!("Removing!");
								for i in 0..sample_size*50000 {
									cons.try_pop();
								}
							} */	

							for i in 0..output.len() {
								if let Some(e) = cons.try_pop() {
									output[i] = e;
								} else {
									output[i] = 0.0;
								}

								//output[i] = 
								//_buffer.lock().unwrap().fill_output(output);
							}
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
				std::thread::sleep(std::time::Duration::from_millis(1));
			}
		});

		Self {
			space_id: space_id,
			user_id: user_id,
			//buffer: buffer,
			sample_rate,
			input_tx: tx
		}
	}
}

fn try_recv_data(user_id: Thing, rx: &mut Receiver<Bytes>) -> Option<Vec<u8>> {
	if let Ok(data) = rx.try_recv() {
		//if ev_user_id == user_id {
			Some(data.to_vec())
		//} else {
		//	None
		//}
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
	
	//let data = delune::separate_channels(data, bit_depth);
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
		if ev.user_id.unwrap() == self.user_id {
			if let UserEvent { user_id: _, space_id: _, context_id: _, user_event_type: Some(UserEventType::SpeakBytesEvent(ev)) } = ev {
				let data = ev.data;
				self.input_tx.blocking_send(Bytes::from(data))?;
			}
		}

		Ok(())
	}

	fn try_recv_event(&mut self) -> anyhow::Result<UserEvent> {
		Err(anyhow!("No events are receivable from speaker worker."))
	}
}


