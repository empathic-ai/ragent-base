use bevy::reflect::{DynamicStruct, Enum, EnumInfo, Typed, ReflectRef};
use serde_json::Value;
//use gloo_console as console;
use fancy_regex::Regex;
//use common::prelude::*;
use std::time::SystemTime;
use tokio::runtime::Handle;
use tokio::sync::RwLock;

use uuid::Uuid;
//use wasm_bindgen::{prelude::*, JsCast};
use std::*;
use common::prelude::*;
use std::{cell::RefCell, rc::Rc};
use std::future::Future;
use std::pin::Pin;
use async_channel::{self, Receiver, Sender};
use std::sync::{Arc};
use futures::lock::Mutex;
use bevy::prelude::*;
use substring::Substring;
use empathic_audio::AudioManager;
use crate::prelude::*;
use anyhow::Result;
use nameof::name_of_type;
use futures_util::{Stream, FutureExt, StreamExt, stream};
use std::collections::HashMap;
use anyhow::anyhow;

use async_trait::async_trait;

pub mod agent_worker;
pub use agent_worker::*;

#[cfg(not(target_os = "android"))]
#[cfg(not(target_arch = "wasm32"))]
pub mod converter_worker;
#[cfg(not(target_os = "android"))]
#[cfg(not(target_arch = "wasm32"))]
pub use converter_worker::*;

pub mod microphone_worker;
pub use microphone_worker::*;

pub mod speaker_worker;
pub use speaker_worker::*;

pub mod user_agent;
pub use user_agent::*;

pub mod audio_buffer;
pub use audio_buffer::*;

//use crate::{types::*, NEW_IMAGE, RESPONSE_ERROR};

pub const MESSAGE: &str = "MESSAGE";
pub const ACTION: &str = "ACTION";

const USE_VOICE: bool = true;
const USE_CAMERA: bool = false;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref EMOTIONS: Vec<&'static str> = {
        vec![
            "default",
            "angry",
            "cheerful",
            "excited",
            "friendly",
            "hopeful",
            "sad",
            "shouting",
            "terrified",
            "unfriendly",
            "whispering"
        ]
    };
}

pub enum Role {
    Human,
    Agent
}

#[bevy_trait_query::queryable]
pub trait UserEventWorker {
    fn is_valid_space(&self, space_id: &Thing) -> Result<bool>;

    fn send_event(&mut self, ev: UserEvent) -> Result<()>;

    fn try_recv_event(&mut self) -> Result<UserEvent>;
}

/* 
#[async_trait]
pub trait TestAgent: Send + Sync { //where Self: Send + Sync + Sized + 'static

fn get_user_id(&self) -> String {
    Uuid::new_v4().to_string()
}

fn new_event(&self, task: Dynamic) -> UserEvent {
    UserEvent {
        user_id: self.get_user_id(),
        args: task,
        created_time: Some(SystemTime::now()),
        token: CancellationToken::new()
    }
}

fn run(&mut self) -> (Sender<UserEvent>, Receiver<UserEvent>);

async fn process(&mut self) -> Result<()>;

fn new_message(&mut self, role: Role, text: String);

async fn get_response(&mut self, token: CancellationToken) -> Result<()> {
    Ok(())
}

fn get_task_configs_description(&self, task_configs: String) -> String {
    "".to_string()
}

}*/

pub fn get_function_prompt(task_configs: Vec<TaskConfig>) -> String {
    let available_task_configs: Vec<TaskConfig> = task_configs.iter().filter(|task_config| { task_config.is_available }).cloned().collect();
    let unavailable_task_configs: Vec<TaskConfig> = task_configs.iter().filter(|task_config| { !task_config.is_available }).cloned().collect();

    let available_tasks_description = get_task_configs_description(available_task_configs);
    let unavailable_tasks_description = get_task_configs_description(unavailable_task_configs);

    //log(available_tasks_description.clone());
    //log(unavailable_tasks_description.clone());

    format!(r#"
# You are an agent that has the following functions available:

{}

# When you call a function, write the function in parenthesis like this:

example_function("This is an example of a function!")

# Do not write functions outside of parenthesis. In each repsonse, include one or more functions you call--each function should be separated by a space. Here is an example response:

example_one("A") example_two("B")

# Never write responses like this:

This is an invalid response!

# ...or like this:

(This is also an invalid response.)

# Do not include anything else in the response, other than a set of one or more functions that you take. Again, DO NOT include text outside of the function structure--if you wish to respond without providing a function, use the speak() function and include your response inside of it.

# You will receive responses in the same format as your messages. In other words, responses to your messages will include functions as well--which will represent actions executed beyond your own control. Responses may incldue any of the functions previously listed and also the following functions:

{}"#, available_tasks_description, unavailable_tasks_description)
}

pub fn get_task_configs_description(task_configs: Vec<TaskConfig>) -> String {
    task_configs.iter().map(|task_config| {
        let parameters_description: String = task_config.parameters.iter().map(|parameter| {
            format!("<{}>", parameter.name)
        }).collect::<Vec<_>>().join(", ");

        format!("{}({}) - {}", task_config.name, parameters_description, task_config.description)
    }).collect::<Vec<_>>().join("\n\n")
}


#[async_trait]
pub trait Agent: Send + Sync { //where Self: Send + Sync + Sized + 'static

    fn get_user_id(&self) -> Thing;
    fn get_space_id(&self) -> Thing;

    /*
    fn new_event(&self, task: Dynamic) -> UserEvent {
        UserEvent {
            user_id: self.get_user_id(),
            args: task,
            created_time: Some(SystemTime::now())
        }
    }
    */
    //pub fn process(_commands: Commands, _agent_query: bevy::prelude::Query<(Entity, &mut Agent)>) {
    //}

    //pub fn get_current_image(&self) -> String {
    //    "assets/avatars/".to_string() + &self.state.lock().await.config.name + "/" + &self.state.lock().unwrap().current_emotion + ".png"
    //}

    /* 
    fn send_user_task(&self, user_task: UserEvent) {
        let handle = Handle::current();
        let task_input_tx = self.task_input_tx.clone();
        task_input_tx.send(user_task).await;
        std::thread::spawn(move || {
            // Using Handle::block_on to run async code in the new thread.
            handle.block_on(async move {
                
            });
        });
    }
    */

    async fn process(&mut self) -> Result<()>;

    fn new_message(&mut self, role: Role, content: Content);

    async fn get_response(&mut self, token: CancellationToken) -> Result<()> {
        Ok(())
    }

    /* 
    async fn process_response(&mut self, user_id: Uuid, prompt_text: String, token: CancellationToken) {
        //let user_task_queue = Arc::clone(&self.output_user_tasks);

        // Adds image recognition
        // A system message is sent that describes what the assistant is seeing.

        /*
        if use_camera {

            let camera_description = crate::web::utils::get_camera_description().await.unwrap();
            let _camera_prompt =
                character_name + " sees what appears to be: " + &camera_description;

            let message = chat_completions::ChatCompletionMessage {
                role: Some(chat_completions::MessageRole::system),
                content: Some(camera_description),
            };

            messages.lock().unwrap().push(message);
        }
        */

        println!("Processing prompt: {}", prompt_text.clone());

        let stream = self.get_response().await.unwrap();
        futures::pin_mut!(stream);

        if token.is_cancelled() {
            println!("Is cancelling here...");
            return;
        }

        //let stream = aws_client::AWSClient::new().await.get_response_from_request(chat_completion_request); //Arc::clone(&_self).get_response_async(messages, is_system, first_message_prompt(prompt_text));

        let _sentence_index = 0;
        let _is_speaking = false;
        let _current_emotion = "default".to_string();

        let mut text_tasks = "".to_string();
        let mut full_response = "".to_string();

        let mut function_name = "".to_string();
        let mut function_response = "".to_string();

        while let Some(result) = stream.next().await {
            if token.is_cancelled() {
                println!("Is cancelling...");
                return;
            }
            let mut delta_response = "".to_string();
 
            match result {
                Ok(_response) => {
                    delta_response = _response.delta;
                }
                Err(e) => {
                    panic!("Error getting stream! {}", e);
                    continue;
                }
            }

            text_tasks += &delta_response.clone();

            full_response += &delta_response.clone();

            println!("Full response (loading): {}", &full_response.clone());

            let mut index = 0;
            
            // Process all full commands and returns the last 'dangling command', if any
            text_tasks = self.output_tasks(user_id, text_tasks, false, token.clone()).await;

            if token.is_cancelled() {
                return;
            }
        }

        println!("Full response (finished loading): {}", &full_response.clone());

        self.output_tasks(user_id, text_tasks, true, token.clone()).await;

        if token.is_cancelled() {
            return;
        }

        self.new_message(Role::Agent, full_response);
    }
    */


        /*
    async fn output_task_streams(&mut self, command_name: &str, args: String) {
        //console::info!("Processing temp command: ".to_string() + command_name + "(" + &args + "...)");

        match command_name {
            EMOTE => {}
            SPEAK => {
                self.speak_stream(args).await;
            }
            &_ => {
                //println!("OTHER COMMAND SENT: {}", command_name);
            }
        }
    }
    */

    //fn speak(self: Arc<&mut Self>, sx: async_channel::Sender<Pin<Box<dyn Future<Output = ()>>>>, text: String) {
    //    self.speak_stream(sx, text + " ");
    //}

    fn get_current_token(&self) -> Option<CancellationToken>;

    fn set_current_token(&mut self, token: Option<CancellationToken>);

    fn get_config(&mut self) -> &AgentConfig;

    async fn output_event(&mut self, task: UserEvent) -> Result<()>;

    async fn input_event(&mut self, task: UserEvent) -> Result<()>;

    async fn try_recv_event(&mut self) -> Result<UserEvent>;

    fn wrap_speech_text(&self, speaker: String, speech_text: String) -> String {
        format!(r#"speak("{}")"#, speech_text.trim().trim_matches('\'').trim_matches('"'))
    }
    


    fn parse_partial_json(&self, json_str: &str) -> (bool, Result<Value>) {
        let mut well_formed_json = String::from(json_str);
    
        // Naively try to close open curly braces
        let mut open_braces = well_formed_json.chars().filter(|&c| c == '{').count();
        let close_braces = well_formed_json.chars().filter(|&c| c == '}').count();
        open_braces -= close_braces;
    
        for _ in 0..open_braces {
            well_formed_json.push('}');
        }
    
        (open_braces == 0, serde_json::from_str(&well_formed_json).map_err(|err| anyhow!(err)))
    }
 


    async fn stop(&mut self) -> Result<()>;

}

#[derive(Clone)]
pub struct LLMResponse {
    pub delta: String,
    pub function_call: Option<String>
}

impl LLMResponse {
    pub fn new(delta: String) -> Self {
        LLMResponse { delta: delta, function_call: None }
    }
}

/*
speak(<text>) - The assistant speaks the provided text. Example: speak(How can I help you?)
music(<genre>) - Music plays in the provided genre. Example: music(How can I help you?)
*/


//#[derive(Component)]
pub struct AgentState<T> where T: Agent {
    pub user_id: Uuid,
    pub config: AgentConfig,
    pub is_recording: bool,
    pub use_camera: bool,
    pub current_emotion: String,
    pub task_output_tx: Sender<UserEvent>,
    pub agent: T
}

impl<T> AgentState<T> where T: Agent {

}

/* 
#[derive(Debug)]
pub struct Agent {
    pub user_id: Uuid,
    pub task_input_tx: Sender<UserEvent>,
    pub task_output_rx: Receiver<UserEvent>,
    #[cfg(not(target_arch = "wasm32"))]
    pub state: Arc<Mutex<AgentState<ChatGPT>>>,
    #[cfg(all(target_arch = "wasm32"))]
    pub state: Arc<Mutex<AgentState<Llama>>>
}
*/

        /*
        if USE_VOICE && sentence.trim().chars().count() > 0 {
            let mut ssml_sentence = sentence.clone();

            let current_emotion = self.current_emotion.clone();
            if !current_emotion.is_empty() {
                ssml_sentence = "<mstts:express-as style='".to_string()
                    + &current_emotion
                    + "'>"
                    + &sentence
                    + "</mstts:express-as>";
            }

            //voice_order_id += 1;
            //let _event_sender = sx.clone();

            //let (_tx, _rx) = mpsc::channel::<Pin<Box<dyn Future::<Output = ()>>>>();
            let (_sx, rx) = async_channel::unbounded::<Pin<Box<dyn Future<Output = ()>>>>();
            let voice_name = self.config.azure_voice_name.clone();
            // In another thread, begin loading the action
            spawn({
                async move {
                    let result =
                        AzureTTS::text_to_speech(voice_name.clone(), ssml_sentence.clone()).await;

                    match result {
                        Ok(data) => {
                            AudioRecorder::start_playing(data).await;
                            //sx.send(f).await;
                            //console::info!("Text to speech ran succesfully!");
                        }
                        Err(e) => {
                            //write_event(RESPONSE_ERROR, "error".to_string());
                            panic!("Problem getting speech from text: {:?}", e)
                        }
                    }
                }
            });

            spawn({
                async move {
                    let f = async move {
                        while let result = rx.recv().await {
                            result.unwrap().await;
                            break;
                        }
                    };

                    // TODO: Rework
                    //_event_sender.send(Box::pin(f)).await;
                }
            });
        }
        */
            /* 
            if AppType == AppType::Narration {
                let re = Regex::new("[^\n.!?]*[\n.!?]").unwrap();
                let captures = re.find_iter(&response);
                //if !captures.is_none() {
                //let captures = captures.unwrap();
                //console::info!(captures.count().to_string());
                let mut len = 0;
                for capture in captures {
                    len += capture.unwrap().as_str().chars().count();
                }

                index = len;
            } else {
                */
                /*
                let r = Regex::new("(.*)]:").unwrap();
                let _c = r.captures(&response);
                // Check if this sentance is the beginning of someone talking
                if !_c.is_none() {
                    // If so, remove the name label from the response
                    is_speaking = true;

                    let x = _c.unwrap()[0].to_string();
                    //console::info!(x.clone());
                    response = response.substring(x.chars().count(), response.chars().count()).to_string();
                    //response = "</mstts:express-as><mstts:express-as style='".to_string() + &current_emotion + "'>" + &response;
                } else {
                    continue;
                }
                */

                // Get all current commands

                // Make the temp commands anything remaining from the processed text
                //let _sx = sx.clone();

                //let ___self = Arc::clone(&_self);

                //let __self = Arc::clone(&_self);
                // TODO: Uncomment
                //__self.lock().unwrap().process_commands(_sx);



            /*
                for capture in captures {
                    let mut bracket_text = capture.unwrap().as_str().to_string();
                    let inner_bracket_text = bracket_text.clone().trim_start().trim_end().trim_start_matches("[").trim_end_matches("]").to_string();

                if inner_bracket_text.starts_with("Emotion: ") {
                    current_emotion = inner_bracket_text.trim_start_matches("Emotion: ").trim_start_matches("emotion: ").to_string();
                }

                new_sentence = new_sentence.clone().replace(&(bracket_text.clone() + " "), "");
                display_sentence = display_sentence.replace(&(bracket_text.clone() + " "), "");

                if inner_bracket_text.starts_with("Image: ") || inner_bracket_text.starts_with("image: ") {
                    image_text = inner_bracket_text.trim_start_matches("Image: ").trim_start_matches("image: ").to_string();

                    // Awaits loading the action and then executes on the action
                    let result = generate_image(image_text);
                    match result {
                        Ok(f) => {
                            tx.send(f);
                        }
                        Err(e) => {
                            panic!("Problem getting image from text: {:?}", e)
                        }
                    }
                } else if inner_bracket_text.starts_with("Sound: ") || inner_bracket_text.starts_with("sound: ") {
                    console::info!(inner_bracket_text.clone());
                }
            }
            */


/*
        tokio::task::spawn(//async {

            //let mut full_commands = "".to_string();
            //let mut temp_commands = "".to_string();
            //let messages = Arc::clone(self.Messages);
            //let _messages = Arc::clone(&self.Messages);
            async move {
                Self::process_response(_self, user_id, prompt_text);
        });
 */
        
        //let stream = _self.event_receiver.lock().unwrap();
        //let s = stream.as_mut();
        // Create a priority queue to store the messages
        //let mut message_queue = BinaryHeap::new();

        /*
        spawn({
            let rx = rx.clone();

            async move {
                loop {
                    match rx.try_recv() {
                        Ok(command_func) => {
                            command_func.await;
                        }
                        Err(_e) => {
                            common::utils::sleep(100).await;
                        }
                    }
                }
                /*
                while let result = rx.recv().await {
                    match result {
                        Ok(command_func) => {
                            command_func.await;
                        }
                        Err(e) => {
                            //  The sender is finished/dropped.
                            panic!("Error getting stream! {}", e);
                            //write_event(RESPONSE_ERROR, "error".to_string());
                            break;
                        }
                    }
                }
                */
            }
        });
         */

    /* 
    fn generate_image(description: String) -> Result<Pin<Box<dyn Future<Output = ()>>>, JsValue> {
        let (tx, rx) = mpsc::channel::<String>();

        console::info!("generating image: ", description.clone());
        let ws = WebSocket::new("wss://rm.picfinder.ai/")?;
        ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

        let cloned_ws = ws.clone();
        let _cloned_ws = ws.clone();
        let description = description;

        let onopen_callback = Closure::<dyn FnMut()>::new(move || {
            // Make request for image
            let data = serde_json::ser::to_string(&PicFinderTaskRequest {
                newTask: PicFinderTask {
                    taskUUID: Uuid::new_v4(),
                    startingPage: 0,
                    promptText: format!(
                        "an artistic illustration of {description}, no text, no words, 4k"
                    ),
                    numberResults: 1,
                    sizeId: 5,
                },
            })
            .unwrap();
            //console::info!("sending task request {:?}", data.clone());
            cloned_ws.send_with_str(&data);
        });

        ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
        onopen_callback.forget();

        let onmessage_callback = Closure::<dyn FnMut(_)>::new(move |e: MessageEvent| {
            // Handle difference Text/Binary,...
            if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                console::info!("message event, received Text: {:?}", txt.clone());
                let txt = txt.as_string().unwrap();
                if txt.contains("newImages") {
                    let response: PicFinderNewImagesResponse = serde_json::from_str(&txt).unwrap();
                    let imageSrc = response.newImages.images[0].clone().imageSrc;
                    console::info!("generated image image src: {:?}", imageSrc.clone());
                    tx.send(imageSrc);
                    _cloned_ws.close();
                }
            } else {
                console::info!("message event, received Unknown: {:?}", e.data());
            }
        });

        // set message event handler on WebSocket
        ws.set_onmessage(Some(onmessage_callback.as_ref().unchecked_ref()));
        // forget the callback to keep it alive
        onmessage_callback.forget();

        let f = async move {
            let mut result: Option<String> = None;
            loop {
                result = rx.try_iter().next();
                if result.is_some() {
                    break;
                }
                browser_sleep(100).await;
            }
            write_event(NEW_IMAGE, result.unwrap());
        };

        Ok(Box::pin(f))
    }
*/

    /*
    pub async fn get_response_from_background_async(prompt: String) -> Rc<RefCell<AsyncEventStream<String>>> {
        let _connect_info = JsValue::null();
        //let port : Port = chrome().runtime().connect(Some("lbkolaieainifjfeehojkhgogedlgaia"), connect_info.as_ref().unchecked_ref());

        let id = Uuid::new_v4();

        let async_event_stream = AsyncEventStream::<String>::new();

        let closure = Closure::wrap(Box::new({
            let async_event_stream = Rc::clone(&async_event_stream);
            move |msg: JsValue| {
                //console::info!("Extension page received message:", msg.clone());

                let args: CustomMessage = serde_json::from_str(&msg.as_string().unwrap()).unwrap();

                if args.id == id {
                    async_event_stream
                        .try_borrow_mut()
                        .unwrap()
                        .set_result(args.msg);
                }
            }
        }) as Box<dyn FnMut(JsValue)>);

        let callback = closure.as_ref().unchecked_ref();
        //port.on_message().add_listener(callback);
        chrome().runtime().on_message().add_listener(callback);
        //chrome().runtime().on_message().remove_listener(callback);
        closure.forget();

        console::info!("Extension page sent a message!");
        let msg = CustomMessage { id, msg: prompt };

        let args = &JsValue::from(serde_json::to_string(&msg).unwrap());
        //port.post_message(&JsValue::from(prompt));
        let _x: Result<JsValue, JsValue> = chrome()
            .runtime()
            .send_message(Some("ghilepdgaomldbanikenlglcjmddbelf"), args, None)
            .await;

        async_event_stream
    }
     */

/*
const ssml_prompt: &str = r#"

When the character speaks, write their speech in parenthesis like this: 'speak(This is an example message!)'. Limit the text within speak() to 200 characters or 10 lines. If speech requires more text, add another speak() call after the first one, as in: 'speak(This is one paragraph.) speak(This is another paragraph.)'. Do not speak without wrapping the text within speak(). Include changes in the character's emotional state in parenthesis, as well, such as: 'emote(Happy) speak(Hi there!) emote(Excited) speak(How are you?)'. Only the following emotional states are available: Emote (Default), emote(Angry), emote(Cheerful), emote(Excited), emote(Friendly), emote(Hopeful), emote(Sad), emote(Shouting], emote(Terrified), emote(Unfriendly), emote(Whispering). Here is an example:

emote(Friendly) speak(Hello! Welcome to D's Italian Restaurant. How may I assist you today?) emote(Excited) speak(I'm happy to help!)

Don't use other emotional states than these within parenthesis. Do not write speech outside of parenthesis.

The character has the following actions available:

emote(<emotion>) - The assistant expresses the provided emotion. Example: emote(Sad)

speak(<text>) - The assistant speaks the provided text. Example: speak(How can I help you?)

Google(<search term>) - The assistant searches the web using the provided search term, and awaits results. Example: Google(Pictures of cats)

In each repsonse, include one or more actions the assistant uses. Do not make up results for actions that include results, such as Googling--await results. If you have not yet received results, please indicate that you are still awaiting results."#;
 */


        /*
        for (entity, mut agent) in &mut agent_query {
            while agent.UserTaskQueue.lock().unwrap().len() > 0 {
                let user_task = agent.UserTaskQueue.lock().unwrap().remove(0);
                console::log!(format!("Created new user task named '{}' with args '{}'.", user_task.Task.Name.clone(), user_task.arguments[0].clone()));

                match user_task.Task.Name.as_str() {
                    "Emote" => {
                        agent.emote(user_task.arguments[0].clone());
                        //self.emote(sx, args);
                    }
                    "Speak" => {
                        agent.speak(user_task.arguments[0].clone());
                        //write_event(RESPONSE, );
                        //self.speak(sx, args);
                    }
                    "Google" => {
                        //write_event(ACTION, "Googling '".to_string() + &args + "'.");
                    }
                    &_ => {
                        //console::info!("OTHER COMMAND SENT: ".to_string() + command_name);
                    }
                }
            }
        }
        */

        /*
        if IS_NARRATION {
            if !my_window.WasFirstMessage {
                //play_label.Text = prompt_text.clone();
                prompt_text = r#"Begin writing a transcript for a video narration about "#.to_string() + &prompt_text.clone() + r#", including sounds in brackets (such as [sound: a waterfall] or [sound: pleasant music]) and single sentence descriptions of imagery in brackets (such as [image: a panda eating bamboo]). This is an example:

                [sound: calming music] [image: close up of a puppy]

                This is a narration about dogs. Dogs are excellent pets.

                Use common words in descriptions and limit each description to a sentence between brackets. Don't use unique character names in image descriptions--rather, describe characters in detail every time. Include one image description before every sentence and at the beginning of the transcript. Include many sound descriptions, and try to include a music description at the beginning of the transcript. Write the initial portion of the narration. I will write 'continue' when the narration  should continue. If the end of the narration  is reached, write [End] or respond with [End]:"#;
                my_window.WasFirstMessage = true;
            } else {
                prompt_text = prompt_text.clone();
            }
        }
        */

    /*
    pub fn get_response_async(self: Arc<Self>, messages: Arc<Mutex<Vec<ChatCompletionMessage>>>, is_system: bool, prompt: String) -> async_channel::Receiver::<ChatCompletionResponse> {

        let (mut tx, mut rx) = async_channel::unbounded::<ChatCompletionResponse>();
        let role = if (is_system) { chat_completions::MessageRole::system } else { chat_completions::MessageRole::user };

        let message = chat_completions::ChatCompletionMessage {
            role: Some(role),
            content: Some(prompt),
        };

        messages.lock().unwrap().push(message);

        let req = ChatCompletionRequest {
            model: chat_completions::GPT3_5_TURBO.to_string(),
            messages: messages.lock().unwrap().to_vec(),
            stream: true
        };

        wasm_bindgen_futures::spawn_local({
            async move {
                let url = "https://mmasij7l2jiyuc3vrwlda3ygfi0ygaxm.lambda-url.us-east-2.on.aws/ ".to_string();

                let client = reqwest::Client::new();

                let req = Request::new(req).unwrap();

                let mut stream =
                    client
                    .post(&url)
                    .header(reqwest::header::CONTENT_TYPE, "application/json")
                    .json(&req)
                    .send()
                    .await.unwrap()
                    .bytes_stream()
                    .eventsource();

                //let (mut tx, mut rx) = async_channel::unbounded::<ChatCompletionResponse>();

                //let mut last: String = "null".to_owned();

                while let Some(chunk) = stream.next().await {
                    let chunk = chunk.unwrap().data;
                    if chunk == "[DONE]" {
                        break;
                    } else {
                        // If  it doesn't start with brackets, it's not a message
                        if chunk.clone().starts_with("{") {
                            let result = serde_json::from_str::<ChatCompletionResponse>(&chunk.clone());//.map_err(err::Error::from);

                            //let mut _response: Option<String> = Some("ERROR".to_string());
                            match result {
                                Ok(response) => {
                                    assert_eq!(tx.send(response).await, Ok(()));
                                },//_response = r.choices[0].delta.content.clone(),
                                Err(e) => {
                                    //_response = Some("ERROR".to_string());
                                    panic!("Failed to get response! {}", e);
                                }
                            }
                        }
                    }
                }
            }
        });
        */

    // TODO: Make call to AWS API Gateway w/ bytes passed

    /*
            let role = if (is_system) { chat_completions::MessageRole::system } else { chat_completions::MessageRole::user };

            let message = chat_completions::ChatCompletionMessage {
                role: Some(role),
                content: Some(prompt),
            };

            messages.lock().unwrap().push(message);

            let req = ChatCompletionRequest {
                model: chat_completions::GPT3_5_TURBO.to_string(),
                messages: messages.lock().unwrap().to_vec(),
                stream: true
            };
    */
    /*
    wasm_bindgen_futures::spawn_local({
        async move {
            open_ai_client.chat_completion_incremental(tx, req).await.unwrap();
        }
    });
    */
    //     return rx;
    //}