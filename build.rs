#![allow(warnings)]
use std::{env, path::PathBuf};

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/agents.proto")?;
    Ok(())
}