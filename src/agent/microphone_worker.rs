use std::sync::Mutex;

use super::SpeakBytesEvent;
use super::UserEventWorker;
use super::UserEvent;
use bevy_builder::database::Thing;
use bytes::Bytes;
use cpal::traits::DeviceTrait;
use cpal::traits::HostTrait;
use cpal::traits::StreamTrait;
use cpal::BufferSize;
use cpal::FromSample;
use cpal::Sample;
use cpal::SampleFormat;
use cpal::StreamConfig;
use cpal::SupportedBufferSize;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use bevy::prelude::*;
use anyhow::Result;
use std::sync::Arc;

use ringbuf::{traits::*, HeapRb};
use ringbuf::SharedRb;

#[cfg_attr(feature = "bevy", derive(Component))]
pub struct MicrophoneWorker {
	pub space_id: Thing,
	pub user_id: Thing,
    pub output_rx: Receiver<Bytes>
}

impl MicrophoneWorker {
	pub fn new(space_id: Thing, user_id: Thing) -> Self {
		
		println!("Creating microphone worker...");

		let host = cpal::default_host();
		//for device in host.input_devices().unwrap().into_iter() {
		//	println!("Microphone device: {}", device.name().unwrap());
		//}
		let device = host.input_devices().unwrap().find(|x| x.name().unwrap() == "Microphone (WO Mic Device)").unwrap();

		//let device = host.default_input_device().unwrap();

		// Get supported input formats
		//let mut supported_configs = device.supported_input_configs()
		//.expect("Error querying supported formats");
		println!("Microphone device name: {}", device.name().unwrap());

		let config = device.default_input_config().unwrap();
		let sample_format = config.sample_format();

		if let SupportedBufferSize::Range { min, max } = config.buffer_size() {
			println!("Microphone min frames: {} Microphone max frames {}", min, max);
		}

		let mut config = config.config();
		config.buffer_size = BufferSize::Fixed(10);

		let sample_rate = config.sample_rate.0;
		let channels = config.channels;

		let bits_per_sample = sample_format.sample_size() * 8;

		//let format = supported_configs
        //.find(|f| f.sample_format() == SampleFormat::I16 && f.min_sample_rate().0 <= 16000 && f.max_sample_rate().0 >= 16000).expect("No suitable format found");

		/*
		let config = StreamConfig {
			channels: 2,
			sample_rate: cpal::SampleRate(16000),
			buffer_size: cpal::BufferSize::Default,
		};
	 */
		//let config = device
        //.default_input_config()
        //.expect("Failed to get default input config");
	
		let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(256);
		
		println!("Mic sample rate: {}", config.sample_rate.0);
		println!("Mic channel count: {}", config.channels);
		println!("Mic sample format: {}", sample_format);

		let latency_frames = (150.0 / 1_000.0) * config.sample_rate.0 as f32;
		let latency_samples = latency_frames as usize * config.channels as usize;
		
		let rb = HeapRb::<f32>::new(latency_samples * 2000);
		let (mut prod, mut cons) = rb.split();

		let _space_id = space_id.clone();
		let _user_id = user_id.clone();

		std::thread::spawn(move || {
			loop {
				let mut processing_buffer = vec![0f32; latency_samples*2000];
				let count = cons.pop_slice(&mut processing_buffer);
				if count > 0 {
					println!("Count: {}", count);
					let data = &processing_buffer[..count];
					// Perform resampling and format conversion
					let data = empathic_audio::convert_f32_to_u8(data, 16).unwrap();
					let data = empathic_audio::resample_pcm(data.to_vec(), sample_rate, 16000, channels as u32, 1, 16, 16).unwrap();
					tx.blocking_send(Bytes::from_iter(data.to_vec()));
				}
				// Optional: Add sleep to prevent tight loop
				std::thread::sleep(std::time::Duration::from_millis(200));
			}

			//let _user_id = _user_id.clone();
			//let _space_id = _space_id.clone();
			//
			

			//send_input_data(_user_id, _space_id, &data, &tx);
		});

		std::thread::spawn(move || {
			let err_fn = move |err| {
				eprintln!("an error occurred on stream: {}", err);
			};

			let _space_id = _space_id.clone();
			let _user_id = _user_id.clone();

			let mut chunks: Vec<u8> = vec![];

			let stream = match sample_format {
				cpal::SampleFormat::I8 => {
					todo!()
				},
				cpal::SampleFormat::I16 => {
					device.build_input_stream(
						&config.into(),
						move |data: &[u8], _: &_| {
							//tx.send(Bytes::from_iter(data.to_vec()));
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
					device.build_input_stream(
						&config.into(),
						move |data: &[f32], _: &_| {
							//let data = empathic_audio::separate_channels_f32(data.to_vec(), 2);
							prod.push_slice(data);

							/*
							//println!("GOT VOICE DATA, SENDING");
							let data = empathic_audio::convert_f32_to_u8(data, 16).unwrap();
							let tx = tx.clone();
							std::thread::spawn(move || {
				
								let data = empathic_audio::resample_pcm(data.to_vec(), sample_rate, 16000, channels as u32, 1, 16, 16).unwrap();
								//let _user_id = _user_id.clone();
								//let _space_id = _space_id.clone();
								//
								
								tx.blocking_send(Bytes::from_iter(data.to_vec()));
								//send_input_data(_user_id, _space_id, &data, &tx);
							});
 							*/

							/*
							#[cfg(not(feature = "wasm32"))]
							{
								chunks.extend_from_slice(&data.clone());
								let duration = empathic_audio::get_duration(chunks.len(), 2, 16000, 16);
								if duration > 5.0 {
									let data = empathic_audio::resample_pcm(chunks.to_vec(), 16000, 48000, 2, 2, 16, 16).unwrap();
									let data = empathic_audio::convert_u8_to_f32(&data, 2, 16).unwrap();
									let data = empathic_audio::convert_f32_to_u8(&data, 2, 16).unwrap();

									println!("Output test.wav");
									let bytes = empathic_audio::samples_to_wav(2, 48000, 16, data.clone());
									futures::executor::block_on(common::utils::set_bytes("test.wav", bytes.clone()));
									chunks.clear();
								}
							}
							 */
							
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
				std::thread::sleep(std::time::Duration::from_millis(10));
			}
		});

		Self {
			space_id: space_id,
			user_id: user_id,
			output_rx: rx,
		}
	}
}

fn send_input_data(user_id: Thing, space_id: Thing, input: &[u8], tx: &Sender<UserEvent>)
{
    //tx.blocking_send(UserEvent::new(Some(user_id), space_id, crate::UserEventType::SpeakBytesEvent(SpeakBytesEvent { data: input.to_vec() })));//.ok();
}

impl UserEventWorker for MicrophoneWorker {
	fn is_valid_space(&self, space_id: &Thing) -> Result<bool> {
		Ok(self.space_id == space_id.clone())
	}

	fn send_event(&mut self, ev: UserEvent) -> anyhow::Result<()> {
		Ok(())
	}

	fn try_recv_event(&mut self) -> anyhow::Result<UserEvent> {
        let bytes = self.output_rx.try_recv()?;
		Ok(UserEvent::new(Some(self.user_id.clone()), self.space_id.clone(), crate::UserEventType::SpeakBytesEvent(SpeakBytesEvent { data: bytes.to_vec() })))
	}
}


