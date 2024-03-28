#![allow(warnings)]
#![allow(unused)]

use bevy::reflect::Reflect;
pub use ragent_core;
pub use ragent_derive;
use serde::{Deserialize, Serialize};

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
}

#[derive(Serialize, Deserialize, Reflect, Clone, PartialEq, ::prost::Message)]
pub struct Thing {
    #[prost(string, tag = "1")]
    pub id: ::prost::alloc::string::String
}