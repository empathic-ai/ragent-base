use async_channel::{Sender, Receiver};
use base_agent::{coqui_synthesizer::CoquiSynthesizer};

use bytes::Bytes;
use futures_util::lock::Mutex;

use async_trait::async_trait;

use uuid::Uuid;
use crate::prelude::*;
use std::time::Duration;
use std::*;
use std::collections::HashMap;
use std::sync::Arc;
use futures::Stream;
use futures_util::StreamExt;
use anyhow::{Result, anyhow};
use common::prelude::*;
use empathic_audio::*;
#[cfg(feature = "bevy")]
use bevy::prelude::*;
#[cfg(feature = "bevy")]
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use async_compat::{Compat, CompatExt};
use std::fs::File;
use std::io::{BufReader, Read};
//use rodio::{Decoder, OutputStream, source::Source};

#[cfg_attr(feature = "bevy", derive(Component))]
pub struct AgentWorker {
    pub user_id: Thing,
    pub state: Arc<Mutex<AgentState>>,
}

pub struct AgentState {
    pub user_id: Thing,
    pub space_id: Thing,
    pub transcriber: Option<Box<dyn Transcriber>>,
    pub synthesizer: Option<Box<dyn Synthesizer>>,
    pub messages: Vec<ChatCompletionMessage>,
    pub functions: Vec<Function>,
    pub output_tx: tokio::sync::broadcast::Sender<UserEvent>,
    pub output_rx: tokio::sync::broadcast::Receiver<UserEvent>,
    pub config: AgentConfig,
    pub input_tx: Sender<UserEvent>,
    pub input_rx: Receiver<UserEvent>,
    pub asset_cache: Arc<Mutex<AssetCache>>,
    pub cancel_tx: tokio::sync::broadcast::Sender<()>,
    pub chat_completer: Box<dyn ChatCompleter>,
    pub current_emotion: String,
    pub current_token: Option<CancellationToken>,
    //pub agent_token: CancellationToken,
}

impl AgentWorker {
    pub async fn new(user_id: Thing, space_id: Thing, config: AgentConfig) -> Self {
            
        let system_prompt = config.description.clone();

        let mut functions = Vec::<Function>::new();

        let voice_id = config.voice_id.clone();
        let mic_sample_rate = config.mic_sample_rate.clone();

        let task_configs: Vec<TaskConfig> = config.task_configs_by_name.values().map(|x|x.to_owned()).collect();

        //println!("Initializing agent with system prompt:\n\n{}\n\n", system_prompt.clone());

        let mut messages = Vec::<ChatCompletionMessage>::new();
        
        let message = ChatCompletionMessage {
            name: None,
            role: MessageRole::system,
            content: Content::Text(system_prompt.clone()),
            function_call: None
        };
        messages.push(message);

        let (input_tx, input_rx) = async_channel::unbounded::<UserEvent>();
        let (output_tx, mut output_rx) = tokio::sync::broadcast::channel::<UserEvent>(32);

        let (voice_tx, voice_rx) = async_channel::unbounded::<UserEvent>();

        //let _self = Arc::new(Mutex::new(self));

        let _input_rx = input_rx.clone();
        /*
        tokio::spawn(async move {
            while let Ok(user_task) = _input_rx.recv().await {

                let __self = _self.clone();
                tokio::spawn(async move {
                    __self.lock().await.process_user_task(user_task).await;
                });
            }
        });
        */
        let agent_token = CancellationToken::new();
        let _agent_token = agent_token.clone();
        
        #[cfg(not(feature="server"))]
        #[cfg(not(target_arch="wasm32"))]
        let mut transcriber = whisper_transcriber::WhisperTranscriber::new();
        #[cfg(not(feature="server"))]
        #[cfg(target_arch="wasm32")]
        let mut transcriber = web_speech_transcriber::WebSpeechTranscriber::new();
        #[cfg(feature="server")]
        let mut transcriber = DeepgramTranscriber::new_from_env();

        let (transcriber_input_tx, transcriber_input_rx) = tokio::sync::broadcast::channel::<Bytes>(64);//voice_transcription::channel();

        let _input_tx = input_tx.clone();
        // TODO: Rework with new Bevy implementation
        /*
        let task = tokio::task::spawn(async move {
        );
        */

        let _space_id = space_id.clone();
        tokio::task::spawn(async move {
            
            let mut transcriber_output_rx = transcriber.transcribe_stream(mic_sample_rate, transcriber_input_rx, _agent_token.clone()).await.expect("Transcription error");

            // Using Handle::block_on to run async code in the new thread.
            while let Some(ev) = transcriber_output_rx.next().await {
                let _space_id = _space_id.clone();
                if _agent_token.is_cancelled() {
                    continue;
                }
    
                match ev {
                    Ok(ev) => {
                        if !ev.transcript.trim_start().trim_end().is_empty() {
                            //println!("Sending transcription result to agent!");
                    
                            let speaker = if let Some(speaker) = ev.speaker {
                                &speaker.to_string()
                            } else {
                                "Uknown"
                            };

                            let text = format!("[Speaker:{}] {}", speaker, ev.transcript);
                            let ev = UserEventType::SpeakEvent(SpeakEvent { text: text });
    
                            _input_tx.send(UserEvent::new(Thing { id: "".to_string() }, _space_id, ev)).await.expect("Failed to send speak event");
                        }
                    },
                    Err(err) => {
                        panic!("Transcription error: {}", err)
                    }
                }
            }
        });

        #[cfg(not(feature="server"))]
        let chat_completer = CandleChatCompleter::new();        
        #[cfg(feature="server")]
        //let chat_completer = CandleChatCompleter::new();        
        let chat_completer = ChatGPTChatCompleter::new_from_env();

        let (cancel_tx, mut cancel_rx) = tokio::sync::broadcast::channel::<()>(1);

        let asset_cache = Arc::new(Mutex::new(AssetCache::new()));
        let state = Arc::new(Mutex::new(AgentState {
            user_id: user_id.clone(),
            space_id: space_id.clone(),
            synthesizer: None,
            transcriber: None,
            chat_completer: Box::new(chat_completer),
            messages: messages,
            functions: functions,
            output_tx: output_tx.clone(),
            output_rx: output_tx.subscribe(),
            config: config,
            input_tx: input_tx,
            input_rx: input_rx,
            asset_cache: asset_cache.clone(),
            cancel_tx: cancel_tx,
            current_emotion: "default".to_string(),
            current_token: None
        }));

        let _state = state.clone();

        tokio::task::spawn(async move {
            _state.lock().await.get_some_response().await.expect("Failed to get first response!");

            while let Ok(ev) = _input_rx.recv().await {
                if let Some(UserEventType::SpeakBytesEvent(args)) = ev.user_event_type {
                    //println!("Received audio: {}", args.data.len());
                    transcriber_input_tx.send(Bytes::from_iter(args.data)).unwrap();
                } else {
                    _state.lock().await.process_user_event(ev).await.expect("Failed to get response to user event!");
                }
            }
        });

        let _self = Self {
            user_id: user_id.clone(),
            state: state,
        };

        let _output_tx = output_tx.clone();
        let _asset_cache = asset_cache.clone();
        let _agent_token = agent_token.clone();

        let _space_id = space_id.clone();
        // TODO: Rework with new Bevy implementation
        tokio::task::spawn(async move {
            while let Ok(ev) = voice_rx.recv().await {

                if _agent_token.is_cancelled() {
                    continue;
                }
                let _space_id = _space_id.clone();
                //let token = ev.token.clone();

                //if token.is_cancelled() {
                //    continue;
                //}
                
                if let Some(UserEventType::SpeakResultEvent(event_type)) = ev.user_event_type {
                    let asset = _asset_cache.lock().await.get(event_type.asset_id.clone()).await.expect("Failed to get asset");
                
                    //if token.is_cancelled() {
                    //    continue;
                    //}

                    if !asset.bytes.is_empty() {

                        let bytes = asset.bytes.to_vec();
                        
                        //ev.user_id
                        _output_tx.send(UserEvent::new(ev.user_id.unwrap(), _space_id, UserEventType::SpeakBytesEvent(SpeakBytesEvent { data: bytes }))).unwrap();//.await;

                        //audio_output_tx.send(asset.bytes).await;

                        //let _state = _state.clone();
                        //_state.lock().await.speaker_buffer.write(&bytes);
                        //state.broadcast_ev(_space_id.clone(), UserEvent::new("".to_string(), Dynamic::new(PlayVoiceEvent { data: bytes.to_vec() }), token.clone()), BroadcastMode::HostOnly);
                        //AudioManager::start_playing(bytes.to_vec()).await;
                    }
                }
            } 
        });

        let _output_tx = output_tx.clone();
        let _space_id = space_id.clone();
        tokio::task::spawn(async move {
            tokio::select! {
                _ = Self::run_voice_processing(_output_tx, output_rx, _space_id.clone(), voice_id.clone(), asset_cache.clone(), voice_tx.clone()) => {
  
                },
                _ = cancel_rx.recv() => {
                    println!("Cancelled agent!")
                }
            }
        });
        println!("Initialized agent.");

        //_self.state.lock().await.input_event(UserEvent::new(Thing::new(), space_id.clone(), UserEventType::WaitEvent(WaitEvent {})));

        _self
    }

    async fn run_voice_processing(output_tx: tokio::sync::broadcast::Sender<UserEvent>, mut output_rx: tokio::sync::broadcast::Receiver<UserEvent>, space_id: Thing, voice_id: String, asset_cache: Arc<Mutex<AssetCache>>, voice_tx: async_channel::Sender<UserEvent>) {
        #[cfg(not(feature="server"))]
        let synthesizer = Arc::new(CoquiSynthesizer::new());
        #[cfg(feature="server")]
        //let synthesizer = Arc::new(AzureSynthesizer::new_from_env());
        let synthesizer = Arc::new(ElevenLabsSynthesizer::new_from_env());

        while let Ok(ev) = output_rx.recv().await {

            let _space_id = space_id.clone();

            let ev_description = ev.get_event_description().unwrap();
            //log(format!("Processing event elsewhere: {}", ev_description));        

            let user_id = ev.user_id;
            //let args = ev.args;
            //let token = ev.token;

            //if token.is_cancelled() {
            //    continue;
            //}

            if let Some(UserEventType::SpeakEvent(args)) = ev.user_event_type {
                println!("Processing speak event: {}", args.text.clone());
                //let voice_name = "smexy-frog".to_string();
                let voice_name = voice_id.clone();
                let speech_text = args.text.clone();
                let emotion = "default".to_string();
        
                let _speech_text = speech_text.clone();
        
                let synthesizer = synthesizer.clone();
                let load_func = async move {
                    let result = synthesizer.create_speech(emotion, voice_name, _speech_text.clone()).await?;
        
                    let bytes = result.bytes.to_vec();
                    let bytes = samples_to_wav(1, 24000, 16, bytes);
                    Ok(crate::asset_cache::Asset::new(bytes))
                };

                let asset_id = Uuid::new_v4().to_string();
        
                //let user_id = self.agent.get_user_id().clone();
        
                asset_cache.lock().await.load_asset(asset_id.clone(), load_func, false).await.expect("Asset load error");
        
                voice_tx.send(UserEvent::new(user_id.clone().unwrap(), _space_id.clone(), UserEventType::SpeakResultEvent(SpeakResultEvent{ asset_id: asset_id}))).await;
            } else if let Some(UserEventType::SingEvent(args)) = ev.user_event_type {

                let _output_tx = output_tx.clone();
                tokio::spawn(async move {

                    let song_name = args.song_name.replace(" ", "_").to_string();
                    println!("SINGING SONG: {}", song_name.clone());

                    let mut receiver = empathic_audio::read_wav_chunks(format!("assets/songs/{}_anatra.wav", song_name.clone()), Duration::from_millis(500), 24000, 1).await;
    
                    while let Some(data) = receiver.recv().await {
    
                        let data = empathic_audio::samples_to_wav(1, 24000, 16, data);
    
                        _output_tx.send(UserEvent::new(user_id.clone().unwrap(), _space_id.clone(), UserEventType::SpeakBytesEvent(SpeakBytesEvent{ data: data }))).unwrap();
                    }
                    println!("Done playing song.");
                });

            }
        }
    }

    pub async fn stop(&self) -> Result<()> {
        self.state.lock().await.stop().await
    }

    pub async fn input_event(&mut self, task: UserEvent) -> Result<()> {
        self.state.lock().await.input_event(task).await
    }

    pub async fn try_recv_event(&mut self) -> Result<UserEvent> {
        self.state.lock().await.try_recv_event().await
    }
}

#[async_trait]
impl Agent for AgentState {
    fn get_user_id(&self) -> Thing {
        self.user_id.clone()
    }
    
    fn get_space_id(&self) -> Thing {
        self.space_id.clone()
    }

    async fn stop(&mut self) -> Result<()> {
        self.cancel_tx.send(())?;
        Ok(())
    }
    
    async fn process(&mut self) -> Result<()> {
        Ok(())
    }

    fn new_message(&mut self, role: Role, content: Content) {
        let role =  match role { Role::Agent => MessageRole::assistant, Role::Human => MessageRole::user };

        if let Content::Text(text) = content.clone() {
            if let Some(i) = self.messages.iter().rposition(|x| matches!(x.content, Content::Text { .. })) {
                let mut message = &mut self.messages[i];
    
                if message.role == role {
                    let text_content = (if let Content::Text(text) = message.content.clone() {
                        Some(text)
                    } else {
                        None
                    }).unwrap();
    
                    message.content = Content::Text(text_content + &("\n".to_string() + text.as_str()));
                    return;
                }
            }
        }
  
        let message = ChatCompletionMessage {
            name: None,
            role: role,
            content: content,
            function_call: None
        };
        
        self.messages.push(message);
    }

    async fn get_response(&mut self, token: CancellationToken) -> Result<()> {

        let mut stream = self.chat_completer.get_response(self.messages.to_vec(), self.config.task_configs_by_name.values().cloned().collect()).await?;

        let mut text_tasks = "".to_string();
        let mut full_response = "".to_string();

        while let Some(result) = stream.next().await {
            if token.is_cancelled() {
                return Ok(());
            }

            match result {
                Ok(x) => {
                    let content = x.completion;
                    text_tasks += &content.clone();
                    full_response += &content;
                    //println!("{}", full_response);

                    text_tasks = self.output_tasks(text_tasks, false, token.clone()).await;
                    if token.is_cancelled() {
                        return Ok(());
                    }
                    
                    /*
             
                    if let Some(function_call) = x.choices[0].clone().delta.function_call {
                        if let Some(name) = function_call.name {
                            println!("Name: {}", name);
                            function_name = name;
                        }
                        if let Some(arguments) = function_call.arguments {
                            function_response += &arguments.clone();
                            println!("Arguments: {}", function_response.clone());
                            if let (is_complete, Ok(val)) = Self::parse_partial_json(&function_response) {
                                if is_complete {
                                    if let Some(val) = val.as_object() {
                                        let arguments: HashMap<String, String> = val.iter().map(|(key, val)|(key.to_owned(), val.as_str().unwrap().to_string())).collect();
                                        
                                        self.call_function_with_map(function_name.clone(), arguments, token.clone()).await;
                                        function_name = "".to_string();
                                        function_response = "".to_string();
                                        //for (name, val) in val {
                                        //    println!("Argument: {}", val.as_str().unwrap());
                                        //}
                                    }
                                }
                            }
                        }
                    }
                    */

                    //Ok(LLMResponse {
                    //    delta: x.choices[0].delta.content.clone().unwrap().to_string(),
                    //    function_call: None
                    //})
                },
                Err(err) => {
                    return Err(anyhow!(err));
                }
            }
        }
        
        text_tasks = self.output_tasks(text_tasks, true, token.clone()).await;
                        
        //log(full_response);
        /*
        let stream = stream.map(|x| {
            match x {
      
            }
        });
        */
        Ok(())
    }

    async fn try_recv_event(&mut self) -> Result<UserEvent> {
        let ev = self.output_rx.try_recv()?;
        Ok(ev)
    }

    fn get_current_token(&self) -> Option<CancellationToken> {
        self.current_token.clone()
    }

    fn set_current_token(&mut self, token: Option<CancellationToken>) {
        self.current_token = token;
    }

    fn get_config(&mut self) -> &AgentConfig {
        &self.config
    }

    async fn output_event(&mut self, task: UserEvent) -> Result<()> {
        self.output_tx.send(task)?;
        Ok(())
    }

    async fn input_event(&mut self, task: UserEvent) -> Result<()> {
        self.input_tx.send(task).await.map_err(|err| anyhow!(err))
    }
}