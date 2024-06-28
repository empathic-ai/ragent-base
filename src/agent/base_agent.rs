use async_channel::{Sender, Receiver};
use base_agent::{coqui_synthesizer::CoquiSynthesizer};

use bytes::Bytes;
use futures_util::lock::Mutex;

use async_trait::async_trait;

use time::{Instant, SystemTime};
use tokio::task::JoinHandle;
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
use substring::Substring;
use fancy_regex::Regex;
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
    pub last_image_time: Option<Instant>
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
            current_token: None,
            last_image_time: None
        }));

        let _state = state.clone();

        tokio::task::spawn(async move {
            Self::get_some_response(_state.clone()).await.expect("Failed to get first response!");

            while let Ok(ev) = _input_rx.recv().await {
                if let Some(UserEventType::SpeakBytesEvent(args)) = ev.user_event_type {
                    //println!("Received audio: {}", args.data.len());
                    transcriber_input_tx.send(Bytes::from_iter(args.data)).unwrap();
                } else {
                    Self::process_user_event(_state.clone(), ev).await.expect("Failed to get response to user event!");
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

    
    pub async fn process_user_event(state: Arc<Mutex<AgentState>>, ev: UserEvent) -> Result<()> {

        let ev_name = ev.get_event_name()?;
        let ev_description = ev.get_event_description()?;
        log(format!("Processing event: {}", ev_description));

        // Todo: Abort previous future if new submission is received (see OpenAI playground for recommendations)
       
        //log(format!("AGENT RECEVED TASK OF TYPE {}", user_task.args.0.type_name()));

        // TODO: handle other task types
        //if user_task.AuthorUserId == self.userId {
        //    return;
        //}
        //let _self = Arc::new(self);
        //let _self = Arc::clone(&self);

        let mut is_image = false;

        let content = match ev.user_event_type.as_ref().unwrap() {
            UserEventType::ImageBytesEvent(ev) => {
                if let Some(last_image_time) = state.lock().await.last_image_time {
                    if Instant::now().duration_since(last_image_time) < Duration::from_secs(30) {
                        return Ok(());
                    }
                }
                state.lock().await.last_image_time = Some(Instant::now());
                is_image = true;

                println!("GOT IMAGE EVENT! PROCESSING IMAGE");

                let data = base64::encode(ev.data.clone());
                let v = vec![ImageUrl { r#type: ContentType::image_url, text: None, image_url: Some(ImageUrlType { url: format!("data:image/jpeg;base64,{}",data) }) }];
                Content::ImageUrl(v)
            },
            _ => {
                if !state.lock().await.get_config().task_configs_by_name.contains_key(&ev_name) {
                    println!("Task doesn't exist in agent config!");
                    return Ok(());
                }

                Content::Text(ev_description)    
            }
        };

        //for arg in ev.args.0.as_ref()

        //let speech_task = ev.args.into_reflect().downcast::<SpeakEventArgs>().unwrap();

        let user_id = state.lock().await.get_user_id();
        //let prompt_text = Self::wrap_speech_text(speech_task.voice_name, speech_task.text.clone());
        
        //println!("Processing prompt: {}", prompt_text);

        let role = if ev.user_id.clone().unwrap() == user_id {
            Role::Agent
        } else {
            Role::Human
        };

        //for message in self.messages.to_vec() {
        //    println!("{:?}: {}", message.role.unwrap(), message.content.unwrap());
        //}

        if ev.user_id.unwrap() == user_id {
            return Ok(());
        }

        //let (sx, rx) = async_channel::unbounded::<Pin<Box<dyn Future<Output = ()>>>>();

        //let character_name = self.get_config().name.clone();

        //let is_system = false;

        //let _voice_name = self.config.azure_voice_name.clone();
        //let _uberduck_id = self.config.uberduck_id;

        //let use_camera = self.use_camera;

        //let _self = Arc::clone(&_self);
        //let sx = sx.clone();

        // Gets response from ChatGPT

        //let prompt_text = prompt_text;
        state.lock().await.new_message(role, content);
        if !is_image {
            Self::get_some_response(state).await?;
        }

        Ok(())
    }

    pub async fn get_some_response(state: Arc<Mutex<AgentState>>) -> Result<()> {
        if let Some(token) = state.lock().await.get_current_token() {
            token.cancel();
        }
        let token = CancellationToken::new();
        state.lock().await.set_current_token(Some(token.clone()));

        let mut stream = state.lock().await.chat_completer.get_response( state.lock().await.messages.to_vec(), state.lock().await.config.task_configs_by_name.values().cloned().collect()).await?;
        
        let _: JoinHandle<anyhow::Result<_>> = tokio::spawn(async move {
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

                        text_tasks = state.lock().await.output_tasks(text_tasks, false, token.clone()).await;
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
            
            text_tasks = state.lock().await.output_tasks(text_tasks, true, token.clone()).await;
            Ok(())
        });    

        Ok(())
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

impl AgentState {
    pub async fn output_tasks(&mut self, temp_commands: String, is_end: bool, token: CancellationToken) -> String {
        if temp_commands.is_empty() {
            return "".to_string();
        }

        let mut temp_commands = temp_commands;
        
        if is_end {
            temp_commands += ")";
        }

        //println!("Parsing tasks from response: '{}'.", temp_commands);

        let (text_tasks, mut dangling_text_task) = get_commands(&temp_commands);

        for text_task in text_tasks {
            let r = Regex::new(r#"(.*?)(?=\()"#).unwrap();
            //println!("{}", command.clone());
            let _command = text_task.clone();
            let command_name = r.captures(&_command).unwrap();

            if let Some(command_name) = command_name {
                let task_name = command_name[0].to_string();

                let args = text_task.substring(task_name.chars().count() + 1,text_task.chars().count() - 1).to_string();

                let args = self.parse_arguments(&args);

                if !(task_name == get_event_name_from_type::<VoiceEventArgs>() || task_name == get_event_name_from_type::<SpeakEvent>()) {
                    //self.config.task_configs
                    self.call_function(task_name, args, token.clone()).await;
                    //if let Some(type_info) = Named::default().get_represented_type_info() {
                    //    if let bevy::reflect::TypeInfo::Enum(enum_info) = type_info {
                    //        enum_info.variant(&command_name);
                    //    }
                    //}
                    //println!("Created new user task named '{}' with args '{}'.", command_name, args);
                } else {
                    self.process_speech(task_name, args, token.clone()).await;
                }
            } else {
                //println!("Failed to create command from dangling text: '{}'.", command.clone());
            }
        }

        //println!("Dangling text tasks: {}", dangling_text_task);
        
        let r = Regex::new(r#"(.*?)(?=\()"#).unwrap();
        let mut t = dangling_text_task.clone();
        let mut _t = dangling_text_task.clone();
        let mut dangling_task_name = r.captures(&_t).unwrap();

        // If there is a space later on in the string, process it as a speech command
        if dangling_task_name.is_none() && dangling_text_task.trim_start_matches("\n").trim_start().contains(' ') {
            //dangling_text_task = r#"speak("default", "default", "#.to_string() + &dangling_text_task;
            dangling_text_task = r#"speak("#.to_string() + &dangling_text_task;
            t = dangling_text_task.clone();
            dangling_task_name = r.captures(&t).unwrap();
        }

        if let Some(dangling_task_name) = dangling_task_name {
            let dangling_task_name = dangling_task_name[0].to_string();
      
            //println!("Dangling task name found: {}", dangling_task_name);

            if (dangling_task_name == get_event_name_from_type::<VoiceEventArgs>() || dangling_task_name == get_event_name_from_type::<SpeakEvent>()) {
                let args = dangling_text_task.substring(dangling_task_name.chars().count() + 1, dangling_text_task.chars().count()).to_string();
                let args = self.parse_arguments(&args);

                // TODO: Write more efficiently
                if dangling_task_name == get_event_name_from_type::<VoiceEventArgs>() && (args.len() > 2 && args[2] != "\"" && args[2] != "\'") {
                    dangling_text_task = self.process_speech(dangling_task_name, args.clone(), token.clone()).await;
                } else if dangling_task_name == get_event_name_from_type::<SpeakEvent>() && (args.len() > 0 && args[0] != "\"" && args[0] != "\'") {
                    dangling_text_task = self.process_speech(dangling_task_name, args.clone(), token.clone()).await;
                }
            }
        }
        dangling_text_task
    }

    async fn call_function_with_map(&mut self, name: String, arguments: HashMap<String, String>, token: CancellationToken) -> Result<()> {
        if let Some(task_config) = self.get_config().task_configs_by_name.get(&name) {
            let mut _arguments = Vec::<String>::new();
            
            for parameter in task_config.parameters.clone() {
                let argument = arguments.get(&parameter.name).unwrap();
                _arguments.push(argument.to_owned());
            }
            self.call_function(name, _arguments, token).await?;
        }
        Ok(())
    }

    async fn call_function(&mut self, name: String, arguments: Vec<String>, token: CancellationToken) -> Result<()> {
        let args_description = arguments.join(", ");
        //log(format!("SENDING RESPONSE: {name}({args_description})"));

        if let Some(task_config) = self.get_config().task_configs_by_name.get(&name) {
            let event_type = (task_config.create_task)(arguments)?;

            let ev = UserEvent::new(self.get_user_id().clone(), self.get_space_id().clone(), event_type);
            self.new_message(Role::Agent, Content::Text(ev.get_event_description()?));
            self.output_event(ev.clone()).await?;
            Ok(())
        } else {
            Err(anyhow!("Failed to find task config!"))
        }
    }
    
    async fn process_speech(&mut self, ev_name: String, mut args: Vec<String>, token: CancellationToken) -> String {
        println!("Processing speech: {}", args.join(", "));

        let length = args.len();
        //let re: Regex = Regex::new(r#".*?(?:\n|\r|\.|\?|!|,)"#).unwrap();
        let re: Regex = Regex::new(r#".*?(?:\n|\r|\.|\?|!)"#).unwrap();

        let speech_text = args[length-1].clone();
        let captures = re.find_iter(&speech_text);

        let mut processed_speech: String = "".to_string();
        for sentence in captures {
            let speech_text = sentence.unwrap().as_str().to_string();

            if !speech_text.trim_matches('.').trim_matches(',').trim_matches('!').trim_matches('?').is_empty() && speech_text != "\"" && speech_text != "\"" {
                processed_speech += &speech_text;

                let mut _args = args.clone();
                _args[length-1] = speech_text.clone();
    
                self.call_function(ev_name.clone(), _args, token.clone()).await.expect("Failed to call function");
            }
        }

        let dangling_speech_text = speech_text.substring(processed_speech.len(), speech_text.len()).trim();

        args[length-1] = dangling_speech_text.to_string();

        let mut speech = ev_name.clone() + "(";
        for i in (0..length) {
            let arg = args[i].clone();
            if i == length-1 {
                speech = speech + "\"" + &arg;
            } else {
                speech = speech + "\"" + &arg + "\", ";
            }
        }
        //let speech = format!(r#"speak("{}", "{}", "{}"#, args[0], args[1], dangling_speech_text);

        //log(format!("Processed speech: {}\nDangling: {}", processed_speech.clone(), speech.clone()));

        speech
    }

    fn parse_arguments(&self, input: &str) -> Vec<String> {
        let mut args = Vec::new();
        let mut current_arg: Option<String> = None;
        let mut in_quotes = false;
        let mut in_single_quotes = false;
    
        for c in input.chars() {
            match c {
                '"' => {
                    in_quotes = !in_quotes;
                    if in_quotes {
                        if current_arg.is_none() {
                            current_arg = Some("".to_string());
                        }
                    }
                }
                '\'' => {
                    if !in_quotes {
                        in_single_quotes = true;
                        in_quotes = true;
                    } else {
                        if in_single_quotes {
                            in_quotes = false;
                            in_single_quotes = false;
                        } else {
                            current_arg = Some(current_arg.unwrap() + &c.to_string());
                        }
                    }
                }
                ',' => {
                    if in_quotes {
                        current_arg = Some(current_arg.unwrap() + &c.to_string());
                    } else {
                        args.push(current_arg.unwrap().trim().to_string());
                        current_arg = Some("".to_string());
                    }
                }
                _ => {
                    if current_arg.is_none() {
                        current_arg = Some("".to_string());
                    }
                    current_arg = Some(current_arg.unwrap() + &c.to_string());
                }
            }
        }
        
        // Append the last argument if it's not empty
        if current_arg.is_some() {
            args.push(current_arg.unwrap().trim().to_string());
        }
    
        args
    }
        
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

        match content.clone() {
            Content::Text(text) => {
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
            },
            Content::ImageUrl(image_url) => {
                self.messages.retain(|x| if let Content::Text(_) = x.content.clone() {
                    true
                } else {
                    false
                });
            },
        }

        let message = ChatCompletionMessage {
            name: None,
            role: role,
            content: content,
            function_call: None
        };
        
        self.messages.push(message);
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

fn get_commands(input: &str) -> (Vec<String>, String) {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut paranthesis_count = 0;
    let mut dangling_text = "".to_string();
    let mut in_quotes = false;

    for c in input.trim_start().trim_start_matches('(').chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
                dangling_text.push(c);
            },
            '(' => {
                if in_quotes {
                    current.push(c);
                    dangling_text.push(c);
                } else {
                    //if paranthesis_count > 0 {
                    current.push(c);
                    //}
                    dangling_text.push(c);
                    paranthesis_count += 1;
                }
            }
            ')' => {
                if in_quotes {
                    current.push(c);
                    dangling_text.push(c);
                } else {
                    paranthesis_count -= 1;
                    if paranthesis_count > 0 {
                        current.push(c);
                    } else {
                        current.push(c);
                        result.push(current.trim_start().to_string());
                        current = String::new();

                        dangling_text = "".to_string();
                    }
                }
            }
            _ => {
                current.push(c);
                dangling_text.push(c);
            }
        }
    }
    dangling_text = dangling_text.trim_start().to_string();
    (result, dangling_text)
}