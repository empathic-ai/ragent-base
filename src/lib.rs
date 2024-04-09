#![allow(warnings)]
#![allow(unused)]

pub mod service {
    use ragent_core::prelude::*;
    tonic::include_proto!("ragent");
}

use std::any::Any;

use prelude::{get_event_name_from_type_name, SpeakEvent};
pub use ragent_core;
pub use ragent_derive;

use bevy::reflect::{DynamicStruct, DynamicTypePath, DynamicVariant, Enum, Reflect, ReflectRef, TypeInfo, TypePath, TypeRegistration, TypeRegistry, VariantInfo};
use serde::{Deserialize, Serialize};
pub use crate::service::user_event::UserEventType;
pub use crate::service::UserEvent;

use anyhow::{Result, anyhow};

pub mod agent;
pub mod config;
pub mod tasks;
pub mod asset_cache;
pub mod tools;

pub mod prelude {
    pub use crate::agent::*;
    pub use crate::config::*;
    pub use crate::tasks::*;
    pub use crate::asset_cache::*;

    pub use crate::ragent_derive::*;
    pub use crate::ragent_core::prelude::*;
    pub use crate::tools::*;

    pub use crate::Thing;

    pub use crate::UserEventType;
    pub use crate::UserEvent;
    pub use crate::service::*;
}

#[derive(Serialize, Deserialize, Reflect, Clone, PartialEq, ::prost::Message)]
pub struct Thing {
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String
}

impl UserEventType {
    pub fn from(event_name: String, event_args: Vec<String>) -> Result<UserEventType> {
        let event_type: Option<UserEventType> = None;
        let type_registry = TypeRegistry::default();
        if let Some(type_registration) = type_registry.get_with_type_path(UserEventType::type_path()) {
            if let TypeInfo::Enum(enum_info) = type_registration.type_info() {
                let variant_name = enum_info.variant_names().iter().find(|x| get_event_name_from_type_name(x) == event_name).unwrap();
        
                if let Some(VariantInfo::Struct(struct_variant_info)) = enum_info.variant(variant_name) {
                    println!("{}", struct_variant_info.name());
                    /*
                    let mut data = DynamicStruct::default();
                    for i in 0..event_args.len() {
                        let field = struct_variant_info.field_at(i).expect("Failed to find field at index");
                        data.insert(field.name(), event_args[i].clone());
                    }
        
                    data.set_represented_type(Some(T::type_info()));
                    //let task = data.clone_value();
                    //T::take_from_reflect(reflect)
                    let task = T::from_reflect(&data).unwrap();
                    //return Ok(Dynamic::new(task));
                    DynamicVariant::Struct(data);
                    let _event_type = UserEventType::SpeakEvent(SpeakEvent::default());
                    _event_type.apply();
                    */
                }
            }
        }
        return Err(anyhow!("Failed to create task from name and arguments!"));
   

        /*
        if let TypeInfo::Struct(struct_info) = T::type_info() {
            let mut data = DynamicStruct::default();
            for i in 0..args.len() {
                let field = struct_info.field_at(i).expect("Failed to find field at index");
                data.insert(field.name(), args[i].clone());
            }
            data.set_represented_type(Some(T::type_info()));
            //let task = data.clone_value();
            //T::take_from_reflect(reflect)
            let task = T::from_reflect(&data).unwrap();
            return Ok(Dynamic::new(task));
        }
        Err(anyhow!("Failed to create task"))
         */
    }
}

impl UserEvent {
    pub fn new(user_id: String, ev: UserEventType) -> Self{
        UserEvent {
            user_id,
            user_event_type: Some(ev)
            //args: task,
            //created_time: Some(SystemTime::now())
        }
    }

    pub fn get_event_name(&self) -> Result<String> {
        
        if let Some(event_type) = self.user_event_type.as_ref() {
            if let ReflectRef::Enum(enum_ref) = event_type.as_reflect().reflect_ref() {
                let type_registry = TypeRegistry::default();
                if let TypeInfo::Enum(enum_info) = type_registry.get_type_info(event_type.type_id()).unwrap() {
                    if let Some(variant_info) = enum_info.variant_at(enum_ref.variant_index()) {
                        let variant_name = get_event_name_from_type_name(variant_info.name());
                        return Ok(variant_name.to_string());
                    }
                }
            }
            return Err(anyhow!("Failed to get event type name!"));
        }
        Err(anyhow!("Failed to get event type from user event!"))
    }

    pub fn get_event_description(&self) -> Result<String> {
        let ev_name = self.get_event_name()?;

        let mut field_values = Vec::<Option::<String>>::new();

        // This closely resembles Self.get_event_name(), since we're getting a variant struct type
        if let Some(event_type) = self.user_event_type.as_ref() {
            if let ReflectRef::Enum(enum_ref) = event_type.as_reflect().reflect_ref() {

                if let Some(variant) = enum_ref.field_at(enum_ref.variant_index()) {
                    if let ReflectRef::Struct(args) = variant.reflect_ref() {
                        for field in args.iter_fields() {
                            if let Some(field) = field.downcast_ref::<String>() {
                                field_values.push(Some(field.to_owned()));
                            } else {
                                field_values.push(None);
                            }
                        }
                    }
                }
            }
    
            let args_description: String = field_values.iter()
            .map(|s| format!(r#""{}""#, s.clone().unwrap_or("".to_string())))
            .collect::<Vec<_>>()
            .join(", ");
    
            return Ok(format!("{ev_name}({args_description})"));
        }
        Err(anyhow!("Failed to get event type from user event!"))
    }
}