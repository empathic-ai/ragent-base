use async_channel::{Sender, Receiver};
use base_agent::{coqui_synthesizer::CoquiSynthesizer, piper_synthesizer::PiperSynthesizer};
use bytes::Bytes;
use futures_util::lock::Mutex;

use async_trait::async_trait;

use uuid::Uuid;
use crate::prelude::*;
use std::*;
use std::collections::HashMap;
use std::sync::Arc;
use futures::Stream;
use futures_util::StreamExt;
use anyhow::{Result, anyhow};
use common::prelude::*;
use empathic_audio::*;

pub struct BaseAgent {
    pub transcriber: Option<Box<dyn Transcriber>>,
    pub synthesizer: Option<Box<dyn Synthesizer>>,
    pub chat_completer: Box<dyn ChatCompleter>,
    pub messages: Vec<ChatCompletionMessage>,
    pub functions: Vec<Function>,
    pub output_tx: tokio::sync::broadcast::Sender<UserEvent>,
    pub output_rx: tokio::sync::broadcast::Receiver<UserEvent>,
    pub config: AgentConfig,
    pub current_token: Option<CancellationToken>,
    pub input_tx: Sender<UserEvent>,
    pub input_rx: Receiver<UserEvent>,
    pub transcriber_tx: tokio::sync::broadcast::Sender<Bytes>,
    pub asset_cache: Arc<Mutex<AssetCache>>,
    pub agent_token: CancellationToken
}

impl BaseAgent {
    pub fn new(config: AgentConfig) -> Self {
            
        let system_prompt = config.description.clone();

        let mut functions = Vec::<Function>::new();

        let task_configs: Vec<TaskConfig> = config.task_configs_by_name.values().map(|x|x.to_owned()).collect();

        //println!("Initializing agent with system prompt:\n\n{}\n\n", system_prompt.clone());

        let mut messages = Vec::<ChatCompletionMessage>::new();
        
        let message = ChatCompletionMessage {
            name: None,
            role: MessageRole::system,
            content: system_prompt.clone(),
            function_call: None
        };
        messages.push(message);

        let (input_tx, input_rx) = async_channel::unbounded::<UserEvent>();
        let (output_tx, mut output_rx) = tokio::sync::broadcast::channel::<UserEvent>(32);

        let (voice_tx, voice_rx) = async_channel::unbounded::<UserEvent>();

        //let _self = Arc::new(Mutex::new(self));

        //let _input_rx = input_rx.clone();
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
        
        let mut transcriber = WhisperTranscriber::new();

        let (transcriber_input_tx, transcriber_input_rx) = voice_transcription::channel();

        let _input_tx = input_tx.clone();
        let task = tokio::task::spawn(async move {
            let mut transcriber_output_rx = transcriber.transcribe_stream(16000, transcriber_input_rx, _agent_token.clone()).await.expect("Transcription error");

            // Using Handle::block_on to run async code in the new thread.
            while let Some(ev) = transcriber_output_rx.next().await {

                if _agent_token.is_cancelled() {
                    continue;
                }

                match ev {
                    Ok(ev) => {
                        if !ev.trim_start().trim_end().is_empty() {
                            println!("Sending transcription result to agent!");
                    
                            let ev = UserEvent::new("".to_string(), Dynamic::new(SpeakEventArgs { text: ev }), CancellationToken::new());

                            _input_tx.send(ev).await.expect("Failed to send speak event");
                        }
                    },
                    Err(err) => {
                        panic!("Transcription error: {}", err)
                    }
                }
            }
        });

        let asset_cache = Arc::new(Mutex::new(AssetCache::new()));

        let _self = Self {
            synthesizer: None,
            transcriber: None,
            chat_completer: Box::new(llama::Llama::new()),
            messages: messages,
            functions: functions,
            output_tx: output_tx.clone(),
            output_rx: output_tx.subscribe(),
            config: config,
            current_token: None,
            input_tx: input_tx,
            input_rx: input_rx,
            transcriber_tx: transcriber_input_tx,
            asset_cache: asset_cache.clone(),
            agent_token: agent_token.clone()
        };

        let _output_tx = output_tx.clone();
        let _asset_cache = asset_cache.clone();
        let _agent_token = agent_token.clone();
        
        tokio::task::spawn(async move {
            while let Ok(ev) = voice_rx.recv().await {

                if _agent_token.is_cancelled() {
                    continue;
                }
                
                let _ev = ev.clone();
                let args = ev.args;
                let token = ev.token.clone();

                if token.is_cancelled() {
                    continue;
                }
                
                if let Some(args) = args.clone().cast::<SpeakResultEventArgs>() {
                    let asset = _asset_cache.lock().await.get(args.asset_id.clone()).await.expect("Failed to get asset");
                
                    if token.is_cancelled() {
                        continue;
                    }

                    if !asset.bytes.is_empty() {

                        let bytes = asset.bytes.to_vec();
                        
                        _output_tx.send(UserEvent::new(ev.user_id, Dynamic::new(SpeakBytesEventArgs { bytes }), token)).unwrap();//.await;

                        //audio_output_tx.send(asset.bytes).await;

                        //let _state = _state.clone();
                        //_state.lock().await.speaker_buffer.write(&bytes);
                        //state.broadcast_ev(_space_id.clone(), UserEvent::new("".to_string(), Dynamic::new(PlayVoiceEvent { data: bytes.to_vec() }), token.clone()), BroadcastMode::HostOnly);
                        //AudioManager::start_playing(bytes.to_vec()).await;
                    }
                }
            }
        });

        //let _output_tx = output_rx.clone();
        let _asset_cache = asset_cache.clone();
        let _agent_token = agent_token.clone();

        tokio::task::spawn(async move {
            let synthesizer = Arc::new(PiperSynthesizer::new());

            while let Ok(ev) = output_rx.recv().await {

                let ev_description = ev.to_description();
                log(format!("Processing event elsewhere: {}", ev_description));        

                if _agent_token.is_cancelled() {
                    continue;
                }

                let user_id = ev.user_id;
                let args = ev.args;
                let token = ev.token;

                if token.is_cancelled() {
                    continue;
                }

                if let Some(args) = args.clone().cast::<SpeakEventArgs>() {
                    //let voice_name = "smexy-frog".to_string();
                    let voice_name = "smexy-frog".to_string();
                    let speech_text = args.text.clone();
                    let emotion = "default".to_string();
            
                    let _speech_text = speech_text.clone();
            
                    let synthesizer = synthesizer.clone();
                    let load_func = async move {
                        let result = synthesizer.create_speech(emotion, voice_name, _speech_text.clone()).await?;
            
                        let bytes = result.bytes.to_vec();
                        let bytes = samples_to_wav(1, 24000, 16, bytes);
                        Ok(Asset::new(bytes))
                    };

                    let asset_id = Uuid::new_v4();
            
                    //let user_id = self.agent.get_user_id().clone();
            
                    asset_cache.lock().await.load_asset(asset_id, load_func, false).await.expect("Asset load error");
            
                    voice_tx.send(UserEvent::new(user_id, Dynamic::new(SpeakResultEventArgs { asset_id: asset_id}), token.clone())).await;
                }
            }
        });
        
        println!("Initialized agent.");

        _self
    }
}

#[async_trait]
impl Agent for BaseAgent {
    async fn stop(&mut self) -> Result<()> {
        self.agent_token.cancel();
        Ok(())
    }
    
    async fn process(&mut self) -> Result<()> {
        if let Ok(ev) = self.input_rx.try_recv() {
            //let __self = _self.clone();
            //tokio::spawn(async move {
            if let Some(args) = ev.args.clone().cast::<SpeakBytesEventArgs>() {
                self.transcriber_tx.send(Bytes::from_iter(args.bytes)).unwrap();
            } else {
                self.process_user_event(ev).await;
            }
            //});
        }
        Ok(())
    }

    fn new_message(&mut self, role: Role, text: String) {
        let message = ChatCompletionMessage {
            name: None,
            role: match role { Role::Agent => MessageRole::assistant, Role::Human => MessageRole::user },
            content: text.clone(),
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
                    println!("{}", full_response);

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