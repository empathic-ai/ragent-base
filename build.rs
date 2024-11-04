#![allow(warnings)]
use std::{env, path::PathBuf};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = tonic_build::configure();

    let mut config = prost_build::Config::new();

    //builder = builder.type_attribute(".ragent.UserEvent", "#[derive(bevy::prelude::Event)]");
    if let Ok(_) = env::var("CARGO_FEATURE_BEVY") {
        config.extern_path(".ragent.Thing", "::bevy_builder::prelude::Thing");
        builder = builder.type_attribute(".", "#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]");
    }

    builder.type_attribute(".", "#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]").compile_with_config(config, &["proto/ragent.proto"], &["proto"])?;

    Ok(())
}