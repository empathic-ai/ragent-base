#![allow(warnings)]
use std::{env, path::PathBuf};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = tonic_build::configure();

    let mut config = prost_build::Config::new();

    //builder = builder.type_attribute(".ragent.UserEvent", "#[derive(bevy::prelude::Event)]");
    config.extern_path(".ragent.Thing", "::flux::prelude::Thing");
    config.extern_path(".ragent.Dynamic", "::flux::prelude::Dynamic");
    
    //builder = builder.client_mod_attribute(".", "use crate::prelude::*;");
    //builder = builder.ser(".", "use crate::prelude::*;");
    //builder = builder.client_attribute(".", "use crate::prelude::*;");
    //builder = builder(".", "use crate::prelude::*;");
        
    //builder = builder.include_file("_includes.rs");

    if let Ok(_) = env::var("CARGO_FEATURE_BEVY") {
        builder = builder.type_attribute(".", "#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]");
    }

    builder.type_attribute(".", "#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]").compile_with_config(config, &["proto/ragent.proto"], &["proto"])?;

    Ok(())
}
