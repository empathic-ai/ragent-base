#![allow(warnings)]
use std::{env, path::PathBuf};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = tonic_build::configure();

    let mut config = prost_build::Config::new();
    config.extern_path(".ragent.Thing", "::ragent::prelude::Thing");

    builder = builder.enum_attribute(".", "#[derive(bevy::prelude::Event)]");

    builder.type_attribute(".", "#[derive(bevy::prelude::Reflect, bevy::prelude::Component, ragent_derive::Task, documented::Documented, serde::Serialize, serde::Deserialize)]").compile_with_config(config, &["proto/ragent.proto"], &["proto"])?;

    Ok(())
}