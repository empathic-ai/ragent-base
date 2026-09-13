fn main() -> Result<(), Box<dyn std::error::Error>> {
    // No protocol work is required for ordinary provider or firmware builds.
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "prost")]
    compile_protocol()?;
    Ok(())
}

#[cfg(feature = "prost")]
fn compile_protocol() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto");
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=PROTOC_INCLUDE");
    let mut config = prost_build::Config::new();
    config.extern_path(".ragent.Thing", "::flux::prelude::Thing");
    config.extern_path(".ragent.Dynamic", "::flux::prelude::Dynamic");
    let attribute = "#[derive(documented::Documented, serde::Serialize, serde::Deserialize)]";

    #[cfg(feature = "tonic")]
    {
        let mut builder = tonic_build::configure();
        if cfg!(feature = "bevy") {
            builder = builder.type_attribute(
                ".",
                "#[derive(bevy::prelude::Reflect, bevy::prelude::Event, ragent_derive::Task)]",
            );
        }
        builder.type_attribute(".", attribute).compile_with_config(
            config,
            &["proto/ragent.proto"],
            &["proto"],
        )?;
    }
    #[cfg(not(feature = "tonic"))]
    config
        .type_attribute(".", attribute)
        .compile_protos(&["proto/ragent.proto"], &["proto"])?;
    Ok(())
}
