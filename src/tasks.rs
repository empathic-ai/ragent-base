use std::{sync::Arc, fmt::Debug};

use crate::prelude::*;

use ragent_core::prelude;

use bevy::{prelude::*, reflect::{Typed, ReflectRef, TypeInfo, ReflectMut, DynamicStruct}};
use bytes::Bytes;
use serde::*;
//use uuid::Uuid;
use std::time::SystemTime;
use documented::Documented;
use anyhow::Result;
use anyhow::anyhow;
use common::prelude::*;

/*
// Ensure that the trait bound includes `Send + Sync` to be thread safe
pub trait CloneBoxedFunc: Fn(String, Vec<String>) -> Result<Dynamic> + Send + Sync {}

// We implement this trait for any function that matches the signature and is 'static, Clone, Send, and Sync
impl<T> CloneBoxedFunc for T
where
    T: 'static + Fn(String, Vec<String>) -> Result<Dynamic> + Clone + Send + Sync,
{
    // We no longer need a custom clone method since Arc will handle it for us
}
*/

/*
// Ensure that the trait bound includes `Send + Sync` to be thread safe
pub trait CloneBoxedCloneFunc: Fn(&Box<dyn Reflect>) -> Box<dyn Reflect> + Send + Sync {}

// We implement this trait for any function that matches the signature and is 'static, Clone, Send, and Sync
impl<T> CloneBoxedCloneFunc for T
where
    T: 'static + Fn(&Box<dyn Reflect>) -> Box<dyn Reflect> + Clone + Send + Sync,
{
    // We no longer need a custom clone method since Arc will handle it for us
}
*/
pub trait CreateTaskFunc: Fn(Vec<String>) -> Result<DynamicStruct> + Send + Sync {}
// We implement this trait for any function that matches the signature and is 'static, Clone, Send, and Sync
impl<T> CreateTaskFunc for T
where
    T: 'static + Fn(Vec<String>) -> Result<DynamicStruct> + Clone + Send + Sync,
{
    // We no longer need a custom clone method since Arc will handle it for us
}

#[derive(Clone)]
pub struct TaskConfig {
    pub name: String,
    pub description: String,
    pub parameters: Vec<Parameter>,
    pub create_task: Arc<dyn CreateTaskFunc>,
    pub is_available: bool
}

#[derive(Clone, Debug)]
pub struct Parameter {
    pub name: String,
    pub description: String
}

impl TaskConfig {
    pub fn new<T>(is_available: bool) -> TaskConfig where T: Task + Typed {
        let docs = T::DOCS;
        let mut parameters = Vec::<Parameter>::new();
        
        if let TypeInfo::Struct(struct_info) = T::type_info() {
            for name in struct_info.field_names() {
              
                /* 
                let description = docs.field_comments.get(name.to_owned());
                let mut _description = "".to_string();
                if let Some(description) = description {
                    _description = description.to_owned();
                }
                */

                parameters.push(Parameter { name: name.to_string(), description: "".to_string() });
            }
        }

        TaskConfig {
            name: get_event_name_from_type::<T>(),
            description: docs.to_string(),
            parameters: parameters,
            create_task: Arc::new(|args: Vec<String>| {
                //UserEventType::from::<T>(args)
                create_task::<T>(args)
            }),
            is_available
        }
    }
}


fn create_task<T>(args: Vec<String>) -> Result<DynamicStruct> where T: Task {
    if let TypeInfo::Struct(struct_info) = T::type_info() {
        let mut data = DynamicStruct::default();
        for i in 0..args.len() {
            let field = struct_info.field_at(i).expect("Failed to find field at index");
            data.insert(field.name(), args[i].clone());
        }
        data.set_represented_type(Some(T::type_info()));
        //let task = data.clone_value();
        //T::take_from_reflect(reflect)
        //let task = T::from_reflect(&data).unwrap();
        return Ok(data);
    }
    Err(anyhow!("Failed to create task"))
}

//impl Default for TaskEvent {
//    fn default() -> Self {
//        TaskEvent::Speak(SpeakEvent::default())
//    }
//}

pub fn get_event_name_from_type<T>() -> String where T: Task {
    let type_name = T::type_info().type_path();
    get_event_name_from_type_name(type_name)
}

/* 
pub fn get_event_name(event_type: UserEventType) -> String {
    if let ReflectRef::Enum(enum_ref) = event_type.as_reflect().reflect_ref() {
        let s = enum_ref.field_at(0).unwrap();
        return s.reflect_type_path().to_string();
    } else {
        panic!("Failed to find event name from instance of event type!");
    }
}*/

pub fn get_event_name_from_type_name(type_name: &str) -> String {
    let mut name = type_name.to_string();//.to_lowercase();
    if let Some(index) = name.rfind("::") {
        name = name.as_str()[index + 2..].to_string(); // +2 to skip the "::"
    }
    name = name.replace("EventArgs", "").replace("Event", "");
    camel_to_snake(&name)
}

fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap());
    }
    result
}

#[derive(Task, Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented)]
/// Speaks text using the provided voice name and emotion
pub struct SpeakEventArgs {
    pub text: String
}

#[derive(Task, Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented)]
/// Speaks text using the provided voice name and emotion
pub struct VoiceEventArgs {
    pub voice_name: String,
    pub emotion: String,
    pub text: String
}

/*
#[derive(Task, Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented)]
/// Represents a non-action--used instead of any other actions if it is most appropriate to wait for further outside input before responding. ONLY use this if explicitly waiting for input from a player.
pub struct WaitEventArgs {
}

#[derive(Task, Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented)]
/// Plays a sound with the provided name
pub struct SoundEventArgs {
    pub name: String
}

#[derive(Task, Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented)]
/// Sends a message including the provided text
pub struct MessageEventArgs {
    pub text: String
}

#[derive(Task, Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented)]
/// Conveys the provided emotion
pub struct EmoteEventArgs {
    pub emotion: String
}

#[derive(Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented, Task)]
/// Includes an audio asset associated with a speak event
pub struct SpeakResultEventArgs {
    pub asset_id: Uuid
}

#[derive(Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented, Task)]
/// Includes an audio asset associated with a speak event
pub struct SpeakBytesEventArgs {
    pub bytes: Vec<u8>
}

#[derive(Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented, Task)]
/// Presents an image based on a description.
pub struct ImageEventArgs {
    pub description: String
}

#[derive(Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented, Task)]
/// Includes an image asset associated with an image event
pub struct ImageResultEventArgs {
    pub asset_id: Uuid
}
*/

/*
pub const MESSAGE: &str = "message";
pub const EMOTE: &str = "emote";
pub const SPEAK_REQUEST: &str = "speak";
pub const SPEAK_RESPONSE: &str = "speak_response";
pub const IMAGE_REQUEST: &str = "image";
pub const IMAGE_RESPONSE: &str = "image_response";
pub const TEXT_ARG: &str = "text";


#[derive(Task, Default, Event, Reflect, Debug, Clone, Serialize, Deserialize, Documented)]
/// Plays music in the provided genre. Use this action to play any music.
pub struct MusicEventArgs {
    pub genre: String
}
*/

/*
// Represents an instance of a task, executed by some user
#[derive(Clone, Debug, Component, Reflect)]
pub struct UserEvent {
    pub user_id: String,
    pub args: Dynamic,
    #[reflect(ignore)] // TODO:
    pub created_time: Option<SystemTime>
}


impl UserEvent {
    pub fn new(user_id: String, task: Dynamic) -> Self{
        UserEvent {
            user_id,
            args: task,
            created_time: Some(SystemTime::now())
        }
    }
}
 */

/*
// Represents a task
#[derive(Debug, Component, Clone, Reflect, Serialize, Deserialize, Default)]
pub struct Task {
    pub name: String,
    #[reflect(ignore)]
    pub description: String,
    pub parameters: Vec<String>,
    #[reflect(ignore)]
    #[serde(skip)]
    pub to_string: Option<fn(UserEvent) -> String>
}

impl Task {
    pub fn message() -> Task {
        Task {
            name: MESSAGE.to_string(),
            description: "".to_string(),
            parameters: vec![ TEXT_ARG.to_string() ],
            to_string: Some(|user_task| {
                "asdf".to_string()
            })
        }
    }

    pub fn speak_simple() -> Task {
        Task {
            name: SPEAK_REQUEST.to_string(),
            description: "The assistant speaks the provided text. Example: speak(How can I help you?). Limit the text within speak() to 200 characters or 10 lines. If speech requires more text, add another speak() call after the first one, as in: 'speak(This is one paragraph.) speak(This is another paragraph.)'.".to_string(),
            parameters: vec![ TEXT_ARG.to_string() ],
            to_string: Some(|user_task| {
                "asdf".to_string()
            })
        }
    }
} */