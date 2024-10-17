#![allow(warnings)]
#![allow(unused)]
#![feature(async_closure)]

pub mod service {
    use ragent_core::prelude::*;
    tonic::include_proto!("ragent");
}

use std::any::Any;

use bevy::ecs::event;
use prelude::{get_event_name_from_type, get_event_name_from_type_name, SpeakEvent};
pub use ragent_core;
pub use ragent_derive;
use bevy::prelude::FromReflect;
use bevy::reflect::{DynamicEnum, DynamicTuple, DynamicTupleStruct, TypeData};
use bevy::reflect::{DynamicStruct, DynamicTypePath, DynamicVariant, Enum, Reflect, ReflectRef, TypeInfo, TypePath, Typed, TypeRegistration, ReflectFromReflect, TypeRegistry, VariantInfo};
use serde::{Deserialize, Serialize};
pub use crate::service::user_event::UserEventType;
pub use crate::service::UserEvent;
pub use bevy_builder::prelude::Thing;

use anyhow::{Result, anyhow};

pub mod agent;
pub mod config;
pub mod tasks;
pub mod asset_cache;
pub mod tools;

use ragent_core::prelude::*;

pub mod prelude {
    pub use crate::agent::*;
    pub use crate::config::*;
    pub use crate::tasks::*;
    pub use crate::asset_cache::*;

    pub use crate::ragent_derive::*;
    pub use crate::ragent_core::prelude::*;
    pub use crate::tools::*;

    pub use crate::UserEventType;
    pub use crate::UserEvent;
    pub use crate::service::*;
    pub use bevy_builder::prelude::Thing;
}



impl UserEventType {
    pub fn from<T>(event_args: Vec<String>) -> Result<UserEventType> where T: Task + Typed {
        let event_name = get_event_name_from_type::<T>();

        //for event_arg in event_args.iter() {
            //println!("Event arg: {}", event_arg);
        //}
    
        if let TypeInfo::Enum(enum_info) = UserEventType::type_info() {
            let variant_name = *enum_info.variant_names().iter().find(|x| get_event_name_from_type_name(x) == event_name).unwrap();
            //println!("Variant name: {}", variant_name);
            
            if let Some(VariantInfo::Tuple(variant_info)) = enum_info.variant(variant_name.clone()) {
                //println!("{}", variant_info.name());
                if let TypeInfo::Struct(struct_info) = T::type_info() {
                    let mut tuple = DynamicTuple::default();

                    let mut data = DynamicStruct::default();
                    for i in 0..event_args.len() {
                        let field = struct_info.field_at(i).expect(&format!("Failed to find field at index {} in type {}. Full args: {:?}", i, event_name, event_args));
                        data.insert(field.name(), event_args[i].clone());
                    }
    
                    data.set_represented_type(Some(T::type_info()));
    
                    tuple.insert_boxed(data.clone_value());
    
                    let dynamic_variant = DynamicVariant::Tuple(tuple);
    
                    let mut dynamic_enum = DynamicEnum::default();
                    dynamic_enum.set_variant(variant_name, dynamic_variant);
    
                    dynamic_enum.set_represented_type(Some(UserEventType::type_info()));

                    return Ok(UserEventType::from_reflect(dynamic_enum.as_reflect()).unwrap());
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

    // Missing struct info inside TupleVariantInfo. Would need to modify Bevy reflect crate to possibly fix
    /*
    pub fn from(event_name: String, event_args: Vec<String>) -> Result<UserEventType> {
        let event_type: Option<UserEventType> = None;

        println!("Event name: {}", event_name);

        if let TypeInfo::Enum(enum_info) = UserEventType::type_info() {
            let variant_name = enum_info.variant_names().iter().find(|x| get_event_name_from_type_name(x) == event_name).unwrap();
            println!("Variant name: {}", variant_name);
            

            if let Some(VariantInfo::Tuple(variant_info)) = enum_info.variant(variant_name) {
                println!("{}", variant_info.name());
                let field = variant_info.field_at(0).unwrap();
                
                let tuple = DynamicTuple::default();

                
                
     
                let mut data = DynamicStruct::default();
                for i in 0..event_args.len() {
                    let field = variant_info.field_at(i).expect("Failed to find field at index");
                    data.insert(field.name(), event_args[i].clone());
                }

                tuple.insert_boxed(data.clone_value());

                DynamicVariant::Tuple(tuple);

                
                UserEventType::from_reflect();
    
                //data.set_represented_type(Some(T::type_info()));
                //let task = data.clone_value();
                //T::take_from_reflect(reflect)
                //let task = T::from_reflect(&data).unwrap();
                //return Ok(Dynamic::new(task));
                DynamicVariant::Struct(data);
                let _event_type = UserEventType::SpeakEvent(SpeakEvent::default());
                _event_type.apply();

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
    */
}

impl UserEvent {
    pub fn new(user_id: Option<Thing>, space_id: Thing, ev: UserEventType) -> Self{
        UserEvent {
            user_id: user_id,
            space_id: Some(space_id),
            context_id: None,
            user_event_type: Some(ev)
        }
    }

    pub fn new_with_context(user_id: Option<Thing>, space_id: Thing, context_id: Thing, ev: UserEventType) -> Self{
        UserEvent {
            user_id: user_id,
            space_id: Some(space_id),
            context_id: Some(context_id),
            user_event_type: Some(ev),
        }
    }

    pub fn get_event_name(&self) -> Result<String> {
        
        if let Some(event_type) = self.user_event_type.as_ref() {
            if let ReflectRef::Enum(enum_ref) = event_type.as_reflect().reflect_ref() {
                if let TypeInfo::Enum(enum_info) = UserEventType::type_info() {
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

                if let Some(variant) = enum_ref.field_at(0) {
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