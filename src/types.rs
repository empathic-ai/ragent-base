use std::path::PathBuf;

#[cfg(feature = "bevy")]
use bevy::prelude::*;
#[cfg(feature = "bevy_reflect")]
use bevy_reflect::{prelude::*, DynamicStruct};
#[cfg(feature = "serde")]
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};
#[cfg(feature = "serde")]
use serde_with::serde_as;
use flux::prelude::*;
use smart_clone::SmartClone;

/// This is a placeholder comment.
#[derive(Reflect, Reactive, documented::Documented, SmartClone)]
#[cfg_attr(feature = "bevy", derive(Event, Component))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Default, Debug)]
pub struct UserEvent {
    pub user_id: Option<Id>,
    pub space_id: Id,
    pub context_id: Option<Id>,
    #[clone(clone_with = "DynamicStruct::to_dynamic_struct")]
    #[serde(with = "dynamic_struct_serde")]
    pub ev: DynamicStruct,
}

/// This is a placeholder comment.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct SpeakBytesEvent {
    pub data: Vec<u8>,
    pub text: Option<String>,
}

/// / Speaks text using the provided voice name and emotion. The text may be a single sentence or multiple sentences.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct SpeakEvent {
    pub text: String,
}
/// / Sets the current emotion of the agent. Call this function prior to speaking if the tone of the agent's voice should be different than the last emotion of the agent.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct EmoteEvent {
    pub text: String,
}
/// / Sings a song with the name provided. Must be one of the songs specified as available, if any.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct SingEvent {
    pub song_name: String,
}

pub fn user_directory(user_id: &Id) -> PathBuf {
    PathBuf::from("assets")
        .join("users")
        .join(user_id.to_pretty_string())
}

pub fn user_songs_directory(user_id: &Id) -> PathBuf {
    user_directory(user_id).join("songs")
}

pub fn user_voice_lines_directory(user_id: &Id) -> PathBuf {
    user_directory(user_id).join("voice-lines")
}

pub fn get_sing_event_prompt(agent_id: &Id) -> String {
    let path = user_songs_directory(agent_id);

    let mut song_names: Vec<String> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|e| e.path().is_file())
        .filter(|e| e.path().extension().is_some_and(|extension| extension == "wav"))
        .filter_map(|e| e.path().file_stem().and_then(|name| name.to_str()).map(String::from))
        .collect();
    song_names.sort_unstable();

    let song_names: String = song_names.iter()
        .map(|s| format!(r#""{}""#, s))
        .collect::<Vec<_>>()
        .join(", ");

    format!("# The following songs are available: {}.", song_names)
}

/// Puts the agent to sleep. Call this function if a user requests the agent to be turned off.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct SleepEvent {}
/// Awakes the agent from sleep. Call this function if a user requests the agent to be turned on after being turned off. The user should specifically say the agent's name for this to be called.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct WakeEvent {}
/// This represents a general system message sent to an agent. An agent will receive system messages if there is some general information it needs to be made aware of.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct SystemEvent {
    pub message: String,
}
/// This represents no response in a conversation. Call this function if no function should be called. Used instead of any other functions if it is most appropriate to wait for further outside input instead of responding. ONLY use this if explicitly waiting for input from a player.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct WaitEvent {}
/// This is a placeholder comment.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct ImageBytesEvent {
    pub data: Vec<u8>,
}
/// This is a placeholder comment.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct SpeakResultEvent {
    pub asset_id: String,
    pub text: String,
}
/// This is a placeholder comment.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct UserJoinedEvent {}
/// This is a placeholder comment.
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct UserLeftEvent {}
/// This action resets the device the agent is running on. Only use this action if prompted to!
#[derive(Reflect, Reactive, ragent_derive::Task, documented::Documented)]
#[cfg_attr(feature = "bevy", derive(Event))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, Debug)]
pub struct ResetDeviceEvent {}
