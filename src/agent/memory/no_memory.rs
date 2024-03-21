use super::MemoryProvider;
use std::collections::HashMap;
use async_trait::async_trait;

pub struct NoMemory {
//    cfg: Config,
}

impl NoMemory {
    pub fn new() -> NoMemory {
        todo!()
    }
}

#[async_trait]
impl MemoryProvider for NoMemory {
    // Implement the methods according to the provided Python code
    async fn add(&mut self, data: String) -> String {
        todo!()
    }

    async fn get(&self, data: String) -> Vec<String> {
        todo!()
    }

    async fn clear(&mut self) -> String {
        todo!()
    }

    async fn get_relevant(&self, data: String, num_relevant: u32) -> Vec<String> {
        todo!()
    }

    async fn get_stats(&self) -> HashMap<String, String> {
        todo!()
    }
}