use async_channel::{Sender, Receiver};
use agent_worker::{coqui_synthesizer::CoquiSynthesizer};

use bytes::Bytes;
use collections::hash_map::Entry;
use futures_lite::StreamExt;
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

use anyhow::{Result, anyhow};
use common::prelude::*;
use delune::*;
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

struct TranscriberWorker {
    pub token: CancellationToken,
    pub input_tx: tokio::sync::broadcast::Sender<Bytes>,
}

impl TranscriberWorker {
    pub async fn new(space_id: Thing, user_id: Option<Thing>, output_tx: tokio::sync::broadcast::Sender<UserEvent>) -> Self {
        #[cfg(not(feature="server"))]
        #[cfg(not(target_arch="wasm32"))]
        #[cfg(not(target_os="android"))]
        let mut transcriber = DeepgramTranscriber::new_from_env();
        #[cfg(not(feature="server"))]
        #[cfg(target_os="android")]
        let mut transcriber = whisper_transcriber::WhisperTranscriber::new();
        #[cfg(not(feature="server"))]
        #[cfg(target_arch="wasm32")]
        let mut transcriber = web_speech_transcriber::WebSpeechTranscriber::new();
        #[cfg(feature="server")]
        let mut transcriber = DeepgramTranscriber::new_from_env();

        //let (input_tx, mut input_rx) = tokio::sync::broadcast::channel::<UserEvent>(32);
        //let (output_tx, mut output_rx) = tokio::sync::broadcast::channel::<UserEvent>(32);

        let (transcriber_input_tx, transcriber_input_rx) = tokio::sync::broadcast::channel::<Bytes>(64);//voice_transcription::channel();

        let token = CancellationToken::new();

        let _token = token.clone();
        //let _output_tx = output_tx.clone();
        
        tokio::task::spawn(async move {
            //println!("Starting transcriber.");

            let mut transcriber_output_rx = transcriber.transcribe_stream(16000, transcriber_input_rx, _token.clone()).await.expect("Transcription error");

            // Using Handle::block_on to run async code in the new thread.
            while let Some(ev) = transcriber_output_rx.next().await {
                //let _space_id = _space_id.clone();
                if _token.is_cancelled() {
                    continue;
                }
    
                match ev {
                    Ok(ev) => {
                        //println!("Got transcript result.");
                        if !ev.transcript.trim_start().trim_end().is_empty() {
                            //println!("Sending transcription result to agent!");
                    
                            let speaker = if let Some(speaker) = ev.speaker {
                                &speaker.to_string()
                            } else {
                                "Uknown"
                            };

                            let text = if user_id.is_none() {
                                format!("[Speaker:{}] {}", speaker, ev.transcript)
                            } else {
                                ev.transcript
                            };
                            output_tx.send(UserEvent::new(user_id.clone(), space_id.clone(), UserEventType::SpeakEvent(SpeakEvent { text: text }))).expect("Failed to send speak event");
                        }
                    },
                    Err(err) => {
                        panic!("Transcription error: {}", err)
                    }
                }
            }
        });

        Self {
            token: token,
            input_tx: transcriber_input_tx,
            //output_rx: output_rx
        }
    }

    pub fn send(&mut self, bytes: Vec<u8>) -> Result<()> {
        //println!("Sending data to transcriber.");
        self.input_tx.send(bytes.into())?;
        Ok(())
    }

    //pub fn try_recv_event(&mut self) -> Result<UserEvent> {
    //    let ev = self.output_rx.try_recv()?;
    //    Ok(ev)
    //}
}


struct ChatCompletionResponseWorker {
    space_id: Thing,
    user_id: Thing,
    context_id: Thing,
    config: AgentConfig,
    messages: Vec<ChatCompletionMessage>,
    chat_completer: Box<dyn ChatCompleter>,
    output_tx: tokio::sync::broadcast::Sender<UserEvent>
}

impl ChatCompletionResponseWorker {
    pub fn new(space_id: Thing, user_id: Thing, context_id: Thing, config: AgentConfig, messages: Vec<ChatCompletionMessage>, output_tx: tokio::sync::broadcast::Sender<UserEvent>, chat_completer: Box<dyn ChatCompleter>) -> Self {
        Self {
            space_id,
            user_id,
            context_id,
            config,
            messages,
            chat_completer,
            output_tx,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let task_configs = self.config.task_configs_by_name.values().cloned().collect();
        
        let mut stream = self.chat_completer.get_response(self.messages.clone(), task_configs).await?;
        
        let mut text_tasks = "".to_string();
        let mut full_response = "".to_string();

        while let Some(result) = stream.next().await {

            match result {
                Ok(x) => {
                    let content = x.completion;
                    text_tasks += &content.clone();
                    full_response += &content;
                   
                    text_tasks = self.output_tasks(text_tasks, false).await?;
                    
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
        
        _ = self.output_tasks(text_tasks, true).await;

        Ok(())  
    }
    
    pub async fn output_tasks(&mut self, temp_commands: String, is_end: bool) -> Result<String> {
        if temp_commands.is_empty() {
            return Ok("".to_string());
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

                let args = Self::parse_arguments(&args);

                if !(task_name == get_event_name_from_type::<VoiceEventArgs>() || task_name == get_event_name_from_type::<SpeakEvent>()) {
                    //self.config.task_configs
                    self.call_function(task_name, args)?;
                    //if let Some(type_info) = Named::default().get_represented_type_info() {
                    //    if let bevy::reflect::TypeInfo::Enum(enum_info) = type_info {
                    //        enum_info.variant(&command_name);
                    //    }
                    //}
                    //println!("Created new user task named '{}' with args '{}'.", command_name, args);
                } else {
                    self.process_speech(task_name, args)?;
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
            dangling_text_task = r#"speak(""#.to_string() + &dangling_text_task.trim_start_matches("\n").trim_start().trim_start_matches("\"");
            t = dangling_text_task.clone();
            dangling_task_name = r.captures(&t).unwrap();
        }

        if let Some(dangling_task_name) = dangling_task_name {
            let dangling_task_name = dangling_task_name[0].to_string();
      
            //println!("Dangling task name found: {}", dangling_task_name);

            if (dangling_task_name == get_event_name_from_type::<VoiceEventArgs>() || dangling_task_name == get_event_name_from_type::<SpeakEvent>()) {
                let args = dangling_text_task.substring(dangling_task_name.chars().count() + 1, dangling_text_task.chars().count()).to_string();
                let args = Self::parse_arguments(&args);

                // TODO: Write more efficiently
                if dangling_task_name == get_event_name_from_type::<VoiceEventArgs>() && (args.len() > 2 && args[2] != "\"" && args[2] != "\'") {
                    dangling_text_task = self.process_speech(dangling_task_name, args.clone())?;
                } else if dangling_task_name == get_event_name_from_type::<SpeakEvent>() && (args.len() > 0 && args[0] != "\"" && args[0] != "\'") {
                    dangling_text_task = self.process_speech(dangling_task_name, args.clone())?;
                }
            }
        }
        Ok(dangling_text_task)
    }
    
    async fn call_function_with_map(&mut self, name: String, arguments: HashMap<String, String>) -> Result<()> {
        if let Some(task_config) = self.config.task_configs_by_name.get(&name) {
            let mut _arguments = Vec::<String>::new();
            
            for parameter in task_config.parameters.clone() {
                let argument = arguments.get(&parameter.name).unwrap();
                _arguments.push(argument.to_owned());
            }
            self.call_function(name, _arguments)?;
        }
        Ok(())
    }

    fn call_function(&mut self, name: String, arguments: Vec<String>) -> Result<()> {
        let args_description = arguments.join(", ");

     
        //log(format!("SENDING RESPONSE: {name}({args_description})"));

        if let Some(task_config) = self.config.task_configs_by_name.get(&name) {
            let event_type = (task_config.create_task)(arguments)?;

            let ev = UserEvent::new_with_context(Some(self.user_id.clone()), self.space_id.clone(), self.context_id.clone(), event_type);
            log(format!("[{}] Generated response: {}", self.config.name, args_description));

            self.output_tx.send(ev.clone())?;
            Ok(())
        } else {
            Err(anyhow!("Failed to find task config!"))
        }
    }

    fn process_speech(&mut self, ev_name: String, mut args: Vec<String>) -> Result<String> {
        //println!("Processing speech: {}", args.join(", "));

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
    
                self.call_function(ev_name.clone(), _args)?;
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

        Ok(speech)
    }
        

    fn parse_arguments(input: &str) -> Vec<String> {
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
}

// TODO: Create more dynamic use of transcribers based on requirements of users within space
// I.e. Realtime API will generate transcriptions itself, whereas other agent types require outside transcription
pub struct SpaceWorker {
    pub space_id: Thing,
    pub token: CancellationToken,
    pub space_transcriber: TranscriberWorker,
    pub user_transcribers: HashMap<Thing, TranscriberWorker>,
    pub output_tx: tokio::sync::broadcast::Sender<UserEvent>,
    pub output_rx: tokio::sync::broadcast::Receiver<UserEvent>,
    pub use_transcribers: bool
}

impl SpaceWorker {
    pub async fn new(space_id: Thing) -> Self {
        //let (input_tx, mut input_rx) = tokio::sync::broadcast::channel::<UserEvent>(32);
        let (output_tx, mut output_rx) = tokio::sync::broadcast::channel::<UserEvent>(32);

        let token = CancellationToken::new();

        Self {
            space_id: space_id.clone(),
            token: token,
            space_transcriber: bevy::tasks::block_on(Compat::new(TranscriberWorker::new(space_id, None, output_tx.clone()))),
            user_transcribers: Default::default(),
            output_tx: output_tx,
            output_rx: output_rx,
            use_transcribers: false
        }
    }

    pub fn send_event(&mut self, ev: UserEvent) -> Result<()> {
        if self.use_transcribers {
            match ev.user_event_type.clone().unwrap() {
                UserEventType::SpeakBytesEvent(speak_bytes_ev) => {
                    if let Some(user_id) = ev.user_id.clone() {
                        //println!("Got user speak bytes, sending to transcriber.");
                        let transcriber = match self.user_transcribers.entry(user_id.clone()) {
                            Entry::Occupied(o) => o.into_mut(),
                            Entry::Vacant(v) => {
                                v.insert(bevy::tasks::block_on(Compat::new(TranscriberWorker::new(self.space_id.clone(), Some(user_id), self.output_tx.clone()))))
                            },
                        };
                        transcriber.send(speak_bytes_ev.data)?;
                    } else {
                        self.space_transcriber.send(speak_bytes_ev.data)?;
                    }
                },
                _ => {
    
                }
            }
        }
        Ok(())
    }

    pub fn try_recv_event(&mut self) -> Result<UserEvent> {
        let ev = self.output_rx.try_recv()?;
        Ok(ev)
    }
}

pub struct AgentState {
    pub user_id: Thing,
    // TODO: Remove this property and enable multiple spaces using Realtime API
    // Agents not using Realtime API don't require this
    pub primary_space_id: Thing,
    pub realtime_api: Option<Box<dyn Realtime>>,
    pub synthesizer: Option<Box<dyn Synthesizer>>,
    pub messages: HashMap<Thing, Vec<ChatCompletionMessage>>,
    pub functions: Vec<Function>,
    pub output_tx: tokio::sync::broadcast::Sender<UserEvent>,
    pub output_rx: tokio::sync::broadcast::Receiver<UserEvent>,
    pub config: AgentConfig,
    pub input_tx: tokio::sync::broadcast::Sender<UserEvent>,
    pub input_rx: tokio::sync::broadcast::Receiver<UserEvent>,
    pub asset_cache: Arc<Mutex<AssetCache>>,
    pub cancel_tx: tokio::sync::broadcast::Sender<()>,
    pub chat_completer: Option<Box<dyn ChatCompleter>>,
    pub current_emotion: String,
    pub last_image_time: Option<Instant>,
    pub user_transcribers: HashMap<Thing, SpaceWorker>,
    pub space_transcribers: HashMap<Thing, SpaceWorker>,
    pub running_contexts: HashMap<Thing, tokio::sync::broadcast::Sender<()>>
    //pub agent_token: CancellationToken,
}

impl AgentWorker {

    pub async fn new(user_id: Thing, primary_space_id: Thing, config: AgentConfig) -> Self {

        let mut functions = Vec::<Function>::new();

        let agent_name = config.name.clone();
        let voice_id = config.voice_id.clone();

        let task_configs: Vec<TaskConfig> = config.task_configs_by_name.values().map(|x|x.to_owned()).collect();
        let system_prompt = config.description.clone();

        let (input_tx, mut input_rx) = tokio::sync::broadcast::channel::<UserEvent>(512);
        let (output_tx, mut output_rx) = tokio::sync::broadcast::channel::<UserEvent>(512);

        let (voice_tx, voice_rx) = async_channel::unbounded::<UserEvent>();

        //let _self = Arc::new(Mutex::new(self));

        let mut _input_rx = input_rx.resubscribe();
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
 
        let _input_tx = input_tx.clone();
        // TODO: Rework with new Bevy implementation
        /*
        let task = tokio::task::spawn(async move {
        );
        */

        //let _space_id = space_id.clone();

        #[cfg(not(feature="server"))]
        #[cfg(not(target_arch="wasm32"))]
        #[cfg(not(target_os="android"))]
        let chat_completer = ChatGPTChatCompleter::new_from_env();
        #[cfg(not(feature="server"))]
        #[cfg(any(target_arch="wasm32", target_os="android"))]
        let chat_completer = CandleChatCompleter::new();        
        #[cfg(feature="server")]
        //let chat_completer = CandleChatCompleter::new();        
        let chat_completer = ChatGPTChatCompleter::new_from_env();

        let (cancel_tx, mut cancel_rx) = tokio::sync::broadcast::channel::<()>(1);

        let asset_cache = Arc::new(Mutex::new(AssetCache::new()));
        let state = Arc::new(Mutex::new(AgentState {
            user_id: user_id.clone(),
            user_transcribers: Default::default(),
            synthesizer: None,
            space_transcribers: Default::default(),
            chat_completer: Some(Box::new(chat_completer)),
            messages: Default::default(),
            functions: functions,
            output_tx: output_tx.clone(),
            output_rx: output_tx.subscribe(),
            config: config,
            input_tx: input_tx,
            input_rx: input_rx.resubscribe(),
            asset_cache: asset_cache.clone(),
            cancel_tx: cancel_tx,
            current_emotion: "default".to_string(),
            last_image_time: None,
            running_contexts: Default::default(),
            realtime_api: None,
            /*
            Some({
                let mut api = ChatGPTRealtime::new_from_env().await;
                api.send(RealtimeEvent::Text(system_prompt)).await;
                api
            }),
            */
            primary_space_id: primary_space_id,
        }));

        let _state = state.clone();

        tokio::task::spawn(async move {
            //Self::get_some_response(_state.clone()).await.expect("Failed to get first response!");
            while let Ok(ev) = _input_rx.recv().await {
                //if let Some(UserEventType::SpeakBytesEvent(args)) = ev.user_event_type {
                    //println!("Received audio: {}", args.data.len());
                //    _state.lock().await.space_transcribers.get(k)
                //    transcriber_input_tx.send(Bytes::from_iter(args.data)).unwrap();
                //} else {
                _state.lock().await.process_input_event(ev).await.expect("Failed to get response to user event!");
                //}
            }
        });

        //Self::new_message(space_id, Role::Agent, Content::Text(ev.get_event_description()?));
        let _self = Self {
            user_id: user_id.clone(),
            state: state.clone(),
        };

        let _output_tx = output_tx.clone();
        let _asset_cache = asset_cache.clone();
        let _agent_token = agent_token.clone();

        let _state = state.clone();
        let _name = agent_name.clone();
        
        //let _space_id = space_id.clone();
        // TODO: Rework with new Bevy implementation
        tokio::task::spawn(async move {
            while let Ok(ev) = voice_rx.recv().await {

                if _agent_token.is_cancelled() {
                    continue;
                }
                let _space_id = ev.space_id.clone().unwrap();
                //let token = ev.token.clone();

                //if token.is_cancelled() {
                //    continue;
                //}
                
                if let Some(UserEventType::SpeakResultEvent(user_ev)) = ev.user_event_type {
                    let asset = _asset_cache.lock().await.get(user_ev.asset_id.clone()).await.expect("Failed to get asset");
                
                    //if token.is_cancelled() {
                    //    continue;
                    //}

                    if !asset.bytes.is_empty() {
                        let is_running = _state.lock().await.is_context_running(ev.context_id.clone().unwrap());
                        if is_running {
                            _state.lock().await.new_message(_space_id.clone(), Role::Agent, Content::Text(user_ev.text.clone()));
                            
                            let bytes = asset.bytes.to_vec();
                            
                            //ev.user_id
                            _output_tx.send(UserEvent::new(ev.user_id, _space_id, UserEventType::SpeakBytesEvent(SpeakBytesEvent { data: bytes }))).unwrap();//.await;
                            //audio_output_tx.send(asset.bytes).await;

                            //let _state = _state.clone();
                            //_state.lock().await.speaker_buffer.write(&bytes);
                            //state.broadcast_ev(_space_id.clone(), UserEvent::new("".to_string(), Dynamic::new(PlayVoiceEvent { data: bytes.to_vec() }), token.clone()), BroadcastMode::HostOnly);
                            //AudioManager::start_playing(bytes.to_vec()).await;
                        } else {
                            println!("[{}] Cancelled voice synthesis.", _name.clone());
                        }
                    }
                }
            } 
        });

        let _output_tx = output_tx.clone();
        let _state = state.clone();
        //let _space_id = space_id.clone();
        tokio::task::spawn(async move {
            tokio::select! {
                _ = Self::process_response(_state, user_id.clone(), _output_tx, input_rx, voice_id.clone(), asset_cache.clone(), voice_tx.clone()) => {
  
                },
                _ = cancel_rx.recv() => {
                    println!("Cancelled agent!")
                }
            }
        });

        info!("Initialized agent: {}", agent_name);

        //_self.state.lock().await.input_event(UserEvent::new(Thing::new(), space_id.clone(), UserEventType::WaitEvent(WaitEvent {})));

        _self
    }


    async fn process_response(state: Arc<Mutex<AgentState>>, user_id: Thing, output_tx: tokio::sync::broadcast::Sender<UserEvent>, mut input_rx: tokio::sync::broadcast::Receiver<UserEvent>, voice_id: String, asset_cache: Arc<Mutex<AssetCache>>, voice_tx: async_channel::Sender<UserEvent>) {
        let mut realtime_api_output_rx = if let Some(api) = &state.lock().await.realtime_api {
            Some(api.get_receiver())
        } else {
            None
        };

        let voice_id = state.lock().await.config.voice_id.clone();
        let name = state.lock().await.config.name.clone();
        let primary_space_id = state.lock().await.primary_space_id.clone();

        if let Some(mut realtime_api_output_rx) = realtime_api_output_rx {
 
            /*
            let mut realtime_buffer = vec![];

            let clear_buffer = async |buffer: &mut Vec<u8>| {
                let converter = super::ElevenLabsConverter::new_from_env();

                let wav_data = delune::samples_to_wav(1, 16000, 16, buffer.to_vec());
                println!("Converting...");
                let result = converter.convert_voice(voice_id.clone(), wav_data).await?;
                println!("Converted.");

                output_tx.send(UserEvent::new(Some(user_id.clone()), primary_space_id.clone(), UserEventType::SpeakBytesEvent(SpeakBytesEvent { data: result.bytes }))).unwrap();
                buffer.clear();
                Ok::<_, anyhow::Error>(())
            };
            */

            while let Ok(ev) = realtime_api_output_rx.recv().await {
                match ev {
                    RealtimeEvent::Text(text) => {

                    }
                    RealtimeEvent::Audio(bytes) => {
                        output_tx.send(UserEvent::new(Some(user_id.clone()), primary_space_id.clone(), UserEventType::SpeakBytesEvent(SpeakBytesEvent { data: bytes.to_vec() }))).unwrap();

                        /*
                        realtime_buffer.append(&mut bytes.to_vec());
                        let duration = delune::get_duration(realtime_buffer.len(), 1, 16000, 16);
                        if duration > 2.0 {
                            clear_buffer(&mut realtime_buffer).await.expect("Failed to clear buffer");
                        }
                        */
                    }
                    RealtimeEvent::AudioEnd => {
                        //let duration = delune::get_duration(realtime_buffer.len(), 1, 16000, 16);
                        //if duration > 0.5 {
                        //    clear_buffer(&mut realtime_buffer).await.expect("Failed to clear buffer");
                        //}
                    }
                }
            }
        } else {
            
            #[cfg(not(feature="server"))]
            #[cfg(feature="game")]
            let synthesizer = Arc::new(ElevenLabsSynthesizer::new_from_env());
            #[cfg(not(feature="server"))]
            #[cfg(not(feature="game"))]
            let synthesizer = Arc::new(CoquiSynthesizer::new());
            #[cfg(feature="server")]
            //let synthesizer = Arc::new(AzureSynthesizer::new_from_env());
            let synthesizer = Arc::new(ElevenLabsSynthesizer::new_from_env());

            while let Ok(ev) = input_rx.recv().await {

                let _space_id = ev.space_id.clone().unwrap();

                let ev_description = ev.get_event_description().unwrap();
                //log(format!("Processing event elsewhere: {}", ev_description));  

                //let args = ev.args;
                //let token = ev.token;

                //if token.is_cancelled() {
                //    continue;
                //}

                if let Some(context_id) = ev.context_id.clone() {
                    if !state.lock().await.is_context_running(context_id) {
                        continue;
                    }
                }
    
                if let Some(_user_id) = ev.user_id.clone() {
                    if _user_id == user_id {
                        if let Some(UserEventType::SpeakEvent(args)) = ev.user_event_type.clone() {
                            //state.lock().await.new_message(_space_id.clone(), Role::Agent, Content::Text(args.text.clone()));
                            output_tx.send(ev.clone());
                    
                            log(format!("[{}] Processing self event: {}", name, ev_description));

                            //println!("Processing speak event: {}", args.text.clone());
                            //let voice_name = "smexy-frog".to_string();
                            let voice_name = voice_id.clone();
                            let speech_text = args.text.clone();
                            let emotion = "default".to_string();
                    
                            let _speech_text = speech_text.clone();
                    
                            let synthesizer = synthesizer.clone();
                            let load_func = async move {
                                let result = synthesizer.create_speech(emotion, voice_name, _speech_text.clone()).await?;
                    
                                let bytes = result.bytes.to_vec();
                                //let bytes = samples_to_wav(1, 24000, 16, bytes);
                                Ok(crate::asset_cache::Asset::new(bytes))
                            };
        
                            let asset_id = Uuid::new_v4().to_string();
                    
                            //let user_id = self.agent.get_user_id().clone();
                    
                            asset_cache.lock().await.load_asset(asset_id.clone(), load_func, false).await.expect("Asset load error");
                    
                            voice_tx.send(UserEvent::new_with_context(Some(user_id.clone()), _space_id.clone(), ev.context_id.clone().unwrap(), UserEventType::SpeakResultEvent(SpeakResultEvent{ asset_id: asset_id, text: speech_text.clone() }))).await;
                            
                        } else if let Some(UserEventType::SingEvent(args)) = ev.user_event_type {
        
                            let _output_tx = output_tx.clone();
                            let _user_id = user_id.clone();
                            tokio::spawn(async move {
        
                                let song_name = args.song_name.replace(" ", "_").to_string();
                                println!("SINGING SONG: {}", song_name.clone());
        
                                let mut receiver = delune::read_wav_chunks(format!("assets/songs/{}_anatra.wav", song_name.clone()), Duration::from_millis(500), AudioFormat::new(24000, 1, 16)).await;
                
                                while let Some(data) = receiver.recv().await {
                
                                    let data = delune::samples_to_wav(1, 24000, 16, data);
                
                                    _output_tx.send(UserEvent::new(Some(_user_id.clone()), _space_id.clone(), UserEventType::SpeakBytesEvent(SpeakBytesEvent{ data: data }))).unwrap();
                                }
                                println!("Done playing song.");
                            });
                        }
                    }
                }
            }   
        }
    }

    pub async fn stop(&self) -> Result<()> {
        self.state.lock().await.stop().await
    }

}

impl UserEventWorker for AgentWorker {
    fn is_valid_space(&self, space_id: &Thing) -> Result<bool> {
        todo!()
    }

    fn send_event(&mut self, task: UserEvent) -> Result<()> {
        bevy::tasks::block_on(async move {
            self.state.lock().await.send_event(task).await
        })
    }

    fn try_recv_event(&mut self) -> Result<UserEvent> {
        bevy::tasks::block_on(async move {
            self.state.lock().await.try_recv_event().await
        })
    }
}

impl AgentState {
    
    pub async fn process_input_event(&mut self, ev: UserEvent) -> Result<()> {

        let user_id = self.get_user_id();
        let ev_user_id = &ev.user_id;

        if ev_user_id.as_ref().unwrap() == &user_id {
            return Ok(());
        }

        let ev_name = ev.get_event_name()?;
        let ev_description = ev.get_event_description()?;
        let space_id = ev.space_id.clone().unwrap();

        if let Some(api) = &mut self.realtime_api {
            if let Some(UserEventType::SpeakBytesEvent(args)) = ev.user_event_type {
                api.send(RealtimeEvent::Audio(Bytes::from(args.data))).await;
            }
        } else {
            let mut is_image = false;

            let content = match ev.user_event_type.as_ref().unwrap() {
                UserEventType::ImageBytesEvent(ev) => {
                    if let Some(last_image_time) = self.last_image_time {
                        if Instant::now().duration_since(last_image_time) < Duration::from_secs(30) {
                            return Ok(());
                        }
                    }
                    self.last_image_time = Some(Instant::now());
                    is_image = true;
    
                    println!("GOT IMAGE EVENT! PROCESSING IMAGE");
    
                    let data = base64::encode(ev.data.clone());
                    let v = vec![ImageUrl { r#type: ContentType::image_url, text: None, image_url: Some(ImageUrlType { url: format!("data:image/jpeg;base64,{}",data) }) }];
                    Content::ImageUrl(v)
                },
                _ => {
                    if !self.get_config().task_configs_by_name.contains_key(&ev_name) {
                        return Ok(());
                    }
    
                    Content::Text(ev_description.clone())    
                }
            };
        
            //for arg in ev.args.0.as_ref()

            //let speech_task = ev.args.into_reflect().downcast::<SpeakEventArgs>().unwrap();

            //let prompt_text = Self::wrap_speech_text(speech_task.voice_name, speech_task.text.clone());
            
            //println!("Processing prompt: {}", prompt_text);

            //for message in self.messages.to_vec() {
            //    println!("{:?}: {}", message.role.unwrap(), message.content.unwrap());
            //}
            
            log(format!("[{}] Processing other ({}) event: {}", self.config.name, ev_user_id.clone().unwrap_or(Thing::from("None")), ev_description));

            //let (sx, rx) = async_channel::unbounded::<Pin<Box<dyn Future<Output = ()>>>>();

            //let character_name = self.get_config().name.clone();

            //let is_system = false;

            //let _voice_name = self.config.azure_voice_name.clone();
            //let _uberduck_id = self.config.uberduck_id;

            //let use_camera = self.use_camera;

            //let _self = Arc::clone(&_self);
            //let sx = sx.clone();

            //let prompt_text = prompt_text;
            self.new_message(space_id.clone(), Role::Human, content);
            if !is_image {
                self.get_chat_completion_response(space_id).await?;
            }
        }

        Ok(())
    }

    pub fn is_context_running(&mut self, context_id: Thing) -> bool {
        self.running_contexts.contains_key(&context_id)
    }

    pub async fn get_chat_completion_response(&mut self, space_id: Thing) -> Result<()> {
        self.running_contexts.retain(|k, mut v| {
            v.send(());
            false
        });

        let (cancel_tx, mut cancel_rx) = tokio::sync::broadcast::channel::<()>(1);

        let context_id = Thing::new();
        self.running_contexts.insert(context_id.clone(), cancel_tx);

        //let x = .unwrap();
        let chat_completer = dyn_clone::clone_box(&*self.chat_completer.as_deref().unwrap());
        let mut response_worker = ChatCompletionResponseWorker::new(space_id.clone(), self.user_id.clone(), context_id, self.config.clone(), self.get_messages(&space_id).clone(), self.input_tx.clone(), chat_completer);

        tokio::task::spawn(async move {
            tokio::select! {
                r = response_worker.run() => {
                    r.expect("Failed to get agent response");
                },
                _ = cancel_rx.recv() => {
                    println!("Cancelled agent repsonse!")
                }
            }
        });

        Ok(())
    }

    pub fn get_messages(&mut self, space_id: &Thing) -> &mut Vec<ChatCompletionMessage> {
        let system_prompt = self.config.description.clone();

        match self.messages.entry(space_id.clone()) {
            Entry::Occupied(o) => {
                o.get()
            },
            Entry::Vacant(v) => {
                            
                let mut messages = Vec::<ChatCompletionMessage>::new();
                
                let message = ChatCompletionMessage {
                    name: None,
                    role: MessageRole::system,
                    content: Content::Text(system_prompt.clone()),
                    function_call: None
                };
                messages.push(message);
                v.insert(messages)
            }
        };
        self.messages.get_mut(&space_id.clone()).unwrap()
    }
    
    fn get_user_id(&self) -> Thing {
        self.user_id.clone()
    }

    async fn stop(&mut self) -> Result<()> {
        self.cancel_tx.send(())?;
        Ok(())
    }
    
    async fn process(&mut self) -> Result<()> {
        Ok(())
    }

    fn new_message(&mut self, space_id: Thing, role: Role, content: Content) {
        let mut messages = self.get_messages(&space_id);

        let role =  match role { Role::Agent => MessageRole::assistant, Role::Human => MessageRole::user };

        match content.clone() {
            Content::Text(text) => {
                if let Some(i) = messages.iter().rposition(|x| matches!(x.content, Content::Text { .. })) {
                    let mut message = &mut messages[i];
        
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
                messages.retain(|x| if let Content::Text(_) = x.content.clone() {
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
        
        messages.push(message);
    }

    async fn try_recv_event(&mut self) -> Result<UserEvent> {
        let ev = self.output_rx.try_recv()?;
        Ok(ev)
    }

    fn get_config(&mut self) -> &AgentConfig {
        &self.config
    }

    async fn output_event(&mut self, task: UserEvent) -> Result<()> {
        //println!("User {} outputting event of type {}.", self.user_id, task.get_event_name()?);
        self.output_tx.send(task)?;
        Ok(())
    }

    async fn send_event(&mut self, task: UserEvent) -> Result<()> {
        self.input_tx.send(task)?;
        Ok(())
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