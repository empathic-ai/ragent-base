#![allow(warnings)]
use std::{env, path::PathBuf};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = prost_build::Config::new();
    config.extern_path(".ragent.Thing", "::flux::prelude::Thing");
    config.extern_path(".ragent.Dynamic", "::flux::prelude::Dynamic");
    
    let attribute = "#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]";
    if let Ok(_) = env::var("CARGO_FEATURE_TONIC") {
        let mut builder = tonic_build::configure();

        if let Ok(_) = env::var("CARGO_FEATURE_BEVY") {
            builder = builder.type_attribute(".", "#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]");
        }

        builder.type_attribute(".", attribute).compile_with_config(config, &["proto/ragent.proto"], &["proto"])?;
    } else {
        config.out_dir(PathBuf::from(std::env::var("OUT_DIR").unwrap()));
        config.type_attribute(".", attribute).compile_protos(&["proto/ragent.proto"], &["proto"]);
    }

    Ok(())
}
