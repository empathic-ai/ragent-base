use super::MemoryProvider;
use std::collections::HashMap;
use ndarray::Array2;
use std::sync::atomic::{AtomicUsize, Ordering};
use async_trait::async_trait;

pub struct LocalCache {
    texts: Vec<String>,
    embeddings: Array2<f32>
}

impl LocalCache {
    pub fn new() -> LocalCache {
        todo!()
    }
}

#[async_trait]
impl MemoryProvider for LocalCache {
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