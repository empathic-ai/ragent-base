use std::collections::HashMap;
use std::sync::{Arc, Mutex};

mod base;
mod local_cache;
mod no_memory;
mod pinecone_memory;
mod redis_memory;

pub use local_cache::LocalCache;
pub use no_memory::NoMemory;
pub use pinecone_memory::PineconeMemory;
pub use redis_memory::RedisMemory;
use async_trait::async_trait;
use std::pin::Pin;

use self::base::MemoryProvider;

pub enum memory_backend {
    pinecone,
    redis,
    local,
    none
}

pub async fn get_memory(backend: memory_backend, init: bool) -> Arc<Mutex<Box<dyn MemoryProvider>>> {
    let mut memory: Box<dyn MemoryProvider> = match backend {
        memory_backend::pinecone => {
            let result = PineconeMemory::new("".to_string(), "".to_string(), "".to_string()).await;
            match result {
                Ok(memory) => {
                    Box::new(memory)
                }
                Err(_) => {
                    panic!("{}", "Failed!");
                }
            }
        },
        memory_backend::redis => {
            Box::new(RedisMemory::new())
        },
        memory_backend::local => {
            Box::new(LocalCache::new())
        },
        memory_backend::none => {
            Box::new(NoMemory::new())
        }
    };
    
    if init {
        memory.clear();
    }

    Arc::new(Mutex::new(memory))
}

pub fn get_supported_memory_backends() -> Vec<String> {
    vec![
        "local".to_string(),
        "redis".to_string(),
        "pinecone".to_string(),
        "no_memory".to_string(),
    ]
}