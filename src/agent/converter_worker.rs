use super::microphone_worker;
use super::speaker_worker;
use super::MicrophoneWorker;
use super::SpeakerWorker;
use super::UserEventWorker;
use super::UserEvent;
use super::VoiceConverter;
use bevy_builder::database::Thing;
use bytes::Bytes;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use bevy::prelude::*;
use anyhow::Result;
use tokio::task::JoinHandle;

#[cfg_attr(feature = "bevy", derive(Component))]
pub struct ConverterWorker {
	pub space_id: Thing,
	pub output_tx: Sender<UserEvent>,
    pub output_rx: Receiver<UserEvent>,
    pub input_tx: Sender<UserEvent>,
    pub input_rx: Receiver<UserEvent>,
	pub handle: JoinHandle<()>
}

impl ConverterWorker {
	pub async fn new(space_id: Thing, user_id: Thing) -> Self {

		let mut microphone_worker = MicrophoneWorker::new(space_id.clone(), user_id.clone(), Some("Microphone (WO Mic Device)".to_string()));
		let mut default_speaker_worker = SpeakerWorker::new(space_id.clone(), user_id.clone(), None);
		let mut loopback_speaker_worker = SpeakerWorker::new(space_id.clone(), user_id.clone(), Some("CABLE Input (VB-Audio Virtual Cable)".to_string()));

		/*
		let handle = tokio::spawn(async move {
			while let Some(data) = microphone_worker.output_rx.recv().await {
				default_speaker_worker.input_tx.send(data).await;
			}
		});
		*/
	
		let mut vad_rx = empathic_audio::streaming::volume_vad_filter(microphone_worker.output_rx);

		let handle = tokio::spawn(async move {
			while let Some(data) = vad_rx.recv().await {
				let converter = super::ElevenLabsConverter::new_from_env();
				let wav_data = empathic_audio::samples_to_wav(1, 16000, 16, data.to_vec());
				//common::utils::set_bytes("pocket_test.wav", wav_data.clone()).await;
				
				let result = converter.convert_voice("vindicta".to_string(), wav_data).await.unwrap();
				//let wav_data = empathic_audio::samples_to_wav(1, 24000, 16, result.bytes);
				//common::utils::set_bytes("pocket_test.wav", wav_data).await;
				let data = Bytes::from_iter(result.bytes);
				loopback_speaker_worker.input_tx.send(data.clone()).await;
				default_speaker_worker.input_tx.send(data).await;
			}
		}); 

		let (tx, rx) = tokio::sync::mpsc::channel::<UserEvent>(32);
		let (_tx, _rx) = tokio::sync::mpsc::channel::<UserEvent>(32);
		
		Self {
			space_id: space_id,
			output_tx: tx,
			output_rx: rx,
			input_tx: _tx,
			input_rx: _rx,
			handle: handle
		}
	}
}

impl UserEventWorker for ConverterWorker {
	fn is_valid_space(&self, space_id: &Thing) -> Result<bool> {
		Ok(self.space_id == space_id.clone())
	}

	fn send_event(&mut self, ev: UserEvent) -> anyhow::Result<()> {
		//todo!()
		Ok(())
	}

	fn try_recv_event(&mut self) -> anyhow::Result<UserEvent> {
        let ev = self.output_rx.try_recv()?;
        Ok(ev)
	}
}


