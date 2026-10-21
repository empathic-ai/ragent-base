use std::path::PathBuf;

#[cfg(feature = "bevy")]
use bevy::prelude::*;
#[cfg(feature = "bevy_reflect")]
use bevy_reflect::{DynamicStruct, prelude::*};
use flux::prelude::*;
#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::DeserializeOwned};
#[cfg(feature = "serde")]
use serde_with::serde_as;
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
    #[serde(with = "history_payload")]
    pub ev: DynamicStruct,
}

pub mod history_payload {
    use super::*;
    use serde::de::Error as _;
    use serde::ser::Error as _;

    const MAX_BYTES: usize = 1_048_576;
    const TYPE_FIELD: &str = "__history_type";
    const DATA_FIELD: &str = "__history_json";

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    pub struct Envelope {
        pub format_version: u16,
        pub type_path: String,
        pub payload_json: String,
    }

    pub fn unknown_type(value: &DynamicStruct) -> Option<&str> {
        if value.get_represented_struct_info().is_some() { return None; }
        value.field(TYPE_FIELD)?.try_downcast_ref::<String>().map(String::as_str)
    }

    fn checked_json(text: &str) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(text.len() <= MAX_BYTES, "History payload exceeds byte limit");
        let mut decoder = serde_json::Deserializer::from_str(text);
        let value = flux::schema::wire::unique_value(&mut decoder)?;
        decoder.end()?;
        Ok(value)
    }

    pub fn encode(value: &DynamicStruct) -> anyhow::Result<Envelope> {
        if let Some(type_path) = unknown_type(value) {
            let payload_json = value.field(DATA_FIELD).and_then(|field| field.try_downcast_ref::<String>())
                .ok_or_else(|| anyhow::anyhow!("Missing unknown history payload"))?.clone();
            let envelope = Envelope { format_version: 1, type_path: type_path.into(), payload_json };
            validate(&envelope)?;
            return Ok(envelope);
        }
        #[derive(Serialize)]
        struct Reflected<'value>(#[serde(with = "dynamic_struct_serde")] &'value DynamicStruct);
        let encoded = serde_json::to_value(Reflected(value))?;
        let object = encoded.as_object().ok_or_else(|| anyhow::anyhow!("Invalid reflected history payload"))?;
        anyhow::ensure!(object.len() == 1, "Invalid reflected history type");
        let (type_path, payload) = object.iter().next().unwrap();
        let envelope = Envelope { format_version: 1, type_path: type_path.clone(), payload_json: serde_json::to_string(payload)? };
        validate(&envelope)?;
        Ok(envelope)
    }

    fn validate(envelope: &Envelope) -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(envelope.format_version == 1, "Unsupported history format");
        anyhow::ensure!(!envelope.type_path.is_empty() && envelope.type_path.len() <= 1024, "Invalid history type");
        let payload = checked_json(&envelope.payload_json)?;
        anyhow::ensure!(payload.is_object(), "History payload must be a struct");
        Ok(payload)
    }

    pub fn decode(envelope: Envelope) -> anyhow::Result<DynamicStruct> {
        use flux::prelude::*;
        let payload = validate(&envelope)?;
        let mut registry = bevy_reflect::TypeRegistry::new();
        registry.register_global_types();
        if let Some(registration) = registry.get_with_type_path(&envelope.type_path) {
            #[derive(Deserialize)]
            struct Reflected(#[serde(with = "dynamic_struct_serde")] DynamicStruct);
            let mut known_payload = payload.clone();
            if let bevy_reflect::TypeInfo::Struct(info) = registration.type_info() {
                known_payload.as_object_mut().unwrap().retain(|name, _| info.field(name).is_some());
            }
            let has_unknown_fields = known_payload != payload;
            let value = serde_json::Value::Object([(envelope.type_path.clone(), known_payload)].into_iter().collect());
            let reflected = serde_json::from_value::<Reflected>(value)?.0;
            if !has_unknown_fields { return Ok(reflected); }
        }
        let mut value = DynamicStruct::default();
        value.insert(TYPE_FIELD, envelope.type_path);
        value.insert(DATA_FIELD, envelope.payload_json);
        Ok(value)
    }

    pub fn serialize<S: Serializer>(value: &DynamicStruct, serializer: S) -> Result<S::Ok, S::Error> {
        encode(value).map_err(S::Error::custom)?.serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<DynamicStruct, D::Error> {
        let envelope = if deserializer.is_human_readable() {
            let value = flux::schema::wire::unique_value(deserializer)?;
            if value.get("format_version").is_some() {
                serde_json::from_value(value).map_err(D::Error::custom)?
            } else {
                let object = value.as_object().ok_or_else(|| D::Error::custom("Invalid legacy history"))?;
                if object.len() != 1 { return Err(D::Error::custom("Invalid legacy history type")); }
                let (type_path, payload) = object.iter().next().unwrap();
                Envelope { format_version: 1, type_path: type_path.clone(), payload_json: serde_json::to_string(payload).map_err(D::Error::custom)? }
            }
        } else { Envelope::deserialize(deserializer)? };
        decode(envelope).map_err(D::Error::custom)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[derive(Serialize, Deserialize)]
        struct History(#[serde(with = "super")] DynamicStruct);

        #[test]
        fn known_history_round_trips_and_rejects_malformed_fields() {
            let event = SpeakEvent { text: "preserved speech".into() };
            let envelope = encode(&event.to_dynamic_struct()).unwrap();
            let type_path = envelope.type_path.clone();
            let restored = decode(envelope).unwrap();
            assert_eq!(restored.field("text").unwrap().try_downcast_ref::<String>().unwrap(), "preserved speech");
            assert!(unknown_type(&restored).is_none());
            assert!(decode(Envelope {
                format_version: 1,
                type_path,
                payload_json: r#"{"text":42}"#.into(),
            }).is_err());
        }

        #[test]
        fn known_history_preserves_unknown_fields() {
            let event = SpeakEvent { text: "speech".into() };
            let mut envelope = encode(&event.to_dynamic_struct()).unwrap();
            envelope.payload_json = r#"{"text":"speech","retired_field":{"value":42}}"#.into();
            let restored = decode(envelope).unwrap();
            assert!(restored.get_represented_struct_info().is_none());
            let encoded = encode(&restored).unwrap();
            assert_eq!(checked_json(&encoded.payload_json).unwrap(),
                serde_json::json!({"text":"speech","retired_field":{"value":42}}));
        }

        #[test]
        fn unknown_history_preserves_payload_without_reflected_type() {
            let payload = r#"{ "future_field": {"values":[1,true,null]} }"#;
            let event = decode(Envelope {
                format_version: 1,
                type_path: "future::UnknownEvent".into(),
                payload_json: payload.into(),
            }).unwrap();
            assert!(event.get_represented_struct_info().is_none());
            assert_eq!(unknown_type(&event), Some("future::UnknownEvent"));
            let encoded = serde_json::to_string(&History(event)).unwrap();
            let restored: History = serde_json::from_str(&encoded).unwrap();
            let envelope = encode(&restored.0).unwrap();
            assert_eq!(envelope.payload_json, payload);
            assert_eq!(envelope.type_path, "future::UnknownEvent");
        }

        #[test]
        fn legacy_unknown_history_remains_readable() {
            let history: History = serde_json::from_str(
                r#"{"legacy::UnknownEvent":{"text":"preserved","extension":42}}"#,
            ).unwrap();
            let envelope = encode(&history.0).unwrap();
            assert_eq!(envelope.type_path, "legacy::UnknownEvent");
            assert_eq!(checked_json(&envelope.payload_json).unwrap(),
                serde_json::json!({"text":"preserved","extension":42}));
        }

        #[test]
        fn history_rejects_duplicate_keys_and_invalid_payloads() {
            for encoded in [
                r#"{"format_version":1,"format_version":1,"type_path":"future::Event","payload_json":"{}"}"#,
                r#"{"legacy::Event":{"text":"first","text":"second"}}"#,
            ] {
                assert!(serde_json::from_str::<History>(encoded).is_err());
            }
            for payload in [r#"{"key":1,"key":2}"#, "{} {}", "[]", "null"] {
                assert!(decode(Envelope {
                    format_version: 1,
                    type_path: "future::Event".into(),
                    payload_json: payload.into(),
                }).is_err());
            }
        }
    }
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
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|extension| extension == "wav")
        })
        .filter_map(|e| {
            e.path()
                .file_stem()
                .and_then(|name| name.to_str())
                .map(String::from)
        })
        .collect();
    song_names.sort_unstable();

    let song_names: String = song_names
        .iter()
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
