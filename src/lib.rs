#![allow(warnings)]
#![allow(unused)]
//#![feature(async_closure)]

#[cfg(feature = "tonic")]
pub mod service {
    use crate::prelude::*;
    //include!(concat!(env!("OUT_DIR"), concat!("\\", "ragent.rs")));
    tonic::include_proto!("ragent");
}

use std::any::Any;

#[cfg(feature = "tonic")]
pub use crate::service::UserEvent;
#[cfg(feature = "tonic")]
pub use crate::service::user_event::UserEventType;

#[cfg(feature = "tokio")]
#[cfg(feature = "futures")]
pub mod asset_cache;

#[cfg(feature = "bevy")]
use bevy::prelude::FromReflect;
#[cfg(feature = "bevy")]
use bevy::prelude::*;
#[cfg(feature = "bevy")]
use bevy::reflect::{DynamicEnum, DynamicTuple, DynamicTupleStruct, TypeData};
#[cfg(feature = "bevy")]
use bevy::reflect::{
    DynamicStruct, DynamicTypePath, DynamicVariant, Enum, Reflect, ReflectFromReflect, ReflectRef,
    TypeInfo, TypePath, TypeRegistration, TypeRegistry, Typed, VariantInfo,
};

#[cfg(feature = "bevy")]
pub use flux::prelude::Id;

#[cfg(feature = "bevy_reflect")]
mod types;
#[cfg(feature = "bevy_reflect")]
use types::*;

//#[cfg(feature = "bevy")]
//use prelude::{get_event_name_from_type, get_event_name_from_type_name, SpeakEvent};
pub use ragent_core;
pub use ragent_derive;
use serde::{Deserialize, Serialize};

use anyhow::{Result, anyhow};

#[cfg(feature = "bevy")]
#[cfg(feature = "futures")]
pub mod agent;
#[cfg(feature = "bevy")]
#[cfg(feature = "futures")]
pub mod config;
#[cfg(feature = "bevy")]
pub mod tasks;
#[cfg(feature = "bevy")]
#[cfg(feature = "futures")]
pub mod tools;

use ragent_core::prelude::*;

pub mod prelude {
    pub use ragent_core::prelude::*;
    pub use ragent_derive::*;

    #[cfg(feature = "tokio")]
    #[cfg(feature = "futures")]
    pub use crate::asset_cache::*;

    #[cfg(feature = "bevy")]
    #[cfg(feature = "futures")]
    pub use crate::agent::*;
    #[cfg(feature = "bevy")]
    #[cfg(feature = "futures")]
    pub use crate::config::*;
    #[cfg(feature = "tonic")]
    pub use crate::service::*;
    #[cfg(feature = "bevy")]
    pub use crate::tasks::*;
    #[cfg(feature = "bevy")]
    #[cfg(feature = "futures")]
    pub use crate::tools::*;
    #[cfg(feature = "bevy_reflect")]
    pub use crate::types::*;
    #[cfg(feature = "bevy")]
    pub use flux::prelude::Id;
}

// Previous module/event experiments are preserved in docs/legacy/lib-experiments.rs.txt.

#[cfg(feature = "bevy")]
impl UserEvent {
    pub fn new<T>(user_id: Id, space_id: Id, ev: T) -> Self
    where
        T: Struct,
    {
        UserEvent {
            user_id: Some(user_id),
            space_id: space_id,
            context_id: None,
            ev: ev.to_dynamic_struct(),
        }
    }

    pub fn new_with_context(user_id: Id, space_id: Id, context_id: Id, ev: DynamicStruct) -> Self {
        UserEvent {
            user_id: Some(user_id),
            space_id: space_id,
            context_id: Some(context_id),
            ev: ev,
        }
    }

    pub fn get_event_name(&self) -> String {
        crate::prelude::get_event_name_from_type_name(
            self.ev
                .get_represented_struct_info()
                .unwrap()
                .type_path_table()
                .short_path(),
        )
        /*
        if let Some(event_type) = self.ev.as_ref() {
            if let ReflectRef::Enum(enum_ref) = event_type.as_reflect().reflect_ref() {
                if let TypeInfo::Enum(enum_info) = UserEventType::type_info() {
                    if let Some(variant_info) = enum_info.variant_at(enum_ref.variant_index()) {
                        let variant_name = crate::prelude::get_event_name_from_type_name(variant_info.name());
                        return Ok(variant_name.to_string());
                    }
                }
            }
            return Err(anyhow!("Failed to get event type name!"));
        }
        Err(anyhow!("Failed to get event type from user event!"))
         */
    }

    pub fn get_event_description(&self) -> Result<String> {
        let ev_name = self.get_event_name();

        let mut field_values = Vec::<Option<String>>::new();

        // This closely resembles Self.get_event_name(), since we're getting a variant struct type
        //if let Some(event_type) = self.ev.as_ref() {
        for field in self.ev.iter_fields() {
            //.reflect_ref() {
            //if let Some(variant) = enum_ref.field_at(0) {
            //if let ReflectRef::Struct(args) = variant.reflect_ref() {
            //for field in args.iter_fields() {
            if let Some(field) = field.try_downcast_ref::<String>() {
                field_values.push(Some(field.to_owned()));
            } else {
                field_values.push(None);
            }
            //}
            //}
            //}
        }

        let args_description: String = field_values
            .iter()
            .map(|s| format!(r#""{}""#, s.clone().unwrap_or("".to_string())))
            .collect::<Vec<_>>()
            .join(", ");

        return Ok(format!("{ev_name}({args_description})"));
        //}
        //Err(anyhow!("Failed to get event type from user event!"))
    }
}
