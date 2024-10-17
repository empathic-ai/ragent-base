use std::process::exit;
use std::env;

use futures::SinkExt;
use futures_util::{future, pin_mut, StreamExt};
use openai_api_rs::realtime::api::RealtimeClient;
use openai_api_rs::realtime::client_event::{ConversationItemCreate, InputAudioBufferAppend, ResponseCreate};
use openai_api_rs::realtime::server_event::ServerEvent;
use openai_api_rs::realtime::types::{Item, ItemContent, ItemType};
use openai_api_rs::realtime::types::ResponseStatusDetail;
use tokio::io::AsyncReadExt;
use tokio_tungstenite::tungstenite::protocol::Message;
//use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use tokio::sync::broadcast::{self, channel, Sender, Receiver};
use bytes::Bytes;

#[derive(Clone)]
pub enum RealtimeEvent {
	Text(String),
	Audio(Bytes),
	AudioEnd
}

pub struct ChatGPTRealtime {
	pub input_tx: Sender<RealtimeEvent>,
	pub output_rx: Receiver<RealtimeEvent>
}

impl ChatGPTRealtime {
    pub async fn new_from_env() -> Self {
        Self::new(env::var("OPENAI_API_KEY").unwrap()).await
    }

	pub async fn send(&mut self, ev: RealtimeEvent) {
		self.input_tx.send(ev);
	}

	pub async fn recv(&mut self) -> RealtimeEvent {
		self.output_rx.recv().await.unwrap()
	}

	pub async fn new(api_key: String) -> Self {
		let model = "gpt-4o-realtime-preview-2024-10-01".to_string();

		//let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();
		//tokio::spawn(read_stdin(stdin_tx));

		let (mut output_tx, output_rx) = channel::<RealtimeEvent>(64);
		let (mut input_tx, mut input_rx) = channel::<RealtimeEvent>(64);

		let realtime_client = RealtimeClient::new(api_key.clone(), model);

		let (mut write, mut read) = realtime_client.connect().await.unwrap();
		println!("WebSocket handshake complete");

		//let stdin_to_ws = stdin_rx.map(Ok).forward(write);

		tokio::task::spawn(async move {
			while let Ok(ev) = &mut input_rx.recv().await {

				match ev {
					RealtimeEvent::Audio(bytes) => {
						//println!("RECEIVED AUDIO EVENT");
						let data = empathic_audio::resample_pcm(bytes.to_vec(), 16000, 24000, 1, 1, 16, 16).unwrap();

						let append_audio_message: Message = InputAudioBufferAppend {
							audio: base64::encode(data),
							..Default::default()
						}.into();
				
						write.send(append_audio_message).await;
					},
					RealtimeEvent::Text(text) => {
						let item_create_message: Message = ConversationItemCreate {
							item: Item {
								r#type: Some(ItemType::Message),
								role: Some(openai_api_rs::realtime::types::ItemRole::User),
								content: Some(vec![
									ItemContent {
										r#type: openai_api_rs::realtime::types::ItemContentType::InputText,
										text: Some(text.clone()),
										audio: None,
										transcript: None
									}
								]),
								..Default::default()
							},
							..Default::default()
						}.into();

						write.send(item_create_message).await;
					}
					_ => {

					}
				}
			}
		});

		tokio::task::spawn(async move {
			while let Some(message) = read.next().await {
				let message = message.unwrap();
				match message {
					Message::Text(text) => {
						let server_event: ServerEvent = serde_json::de::from_str(&text).expect(&format!("Failed to deserialize server event: {}", text));
						match server_event {
							ServerEvent::ResponseAudioDelta(ev) => {
								let bytes = base64::decode(ev.delta).unwrap();
								let bytes = empathic_audio::resample_pcm(bytes, 24000, 16000, 1, 1, 16, 16).unwrap();

								output_tx.send(RealtimeEvent::Audio(Bytes::from(bytes)));
							}
							ServerEvent::ResponseOutputItemDone(ev) => {
								//eprintln!();
							}
							ServerEvent::ResponseAudioTranscriptDelta(ev) => {
								println!("{}", ev.delta.trim());
							}
							ServerEvent::ResponseDone(ev) => {
								if let Some(details) = ev.response.status_details {
									match details {
										ResponseStatusDetail::Incomplete { reason } => {

										},
										ResponseStatusDetail::Failed { error } => {
											eprintln!("Error getting realtime response: {}", error.unwrap().message);
										}
										_ => {
											// Cancelled
										}
									}
								}
								output_tx.send(RealtimeEvent::AudioEnd);
							}
							ServerEvent::ResponseAudioDone(ev) => {
							}
							ServerEvent::Error(e) => {
								eprintln!("{e:?}");
							}
							_ => {}
						}
					}
					Message::Close(_) => {
						eprintln!("Close");
						exit(0);
					}
					_ => {}
				}
			}
		});

		//pin_mut!(stdin_to_ws, ws_to_stdout);
		//future::select(stdin_to_ws, ws_to_stdout).await;

		Self {
			input_tx,
			output_rx
		}
	}
}