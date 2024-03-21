use std::collections::HashMap;
use openairs::{
    client,
    embeddings::{self, EmbeddingsRequest, EmbeddingsResponse}
};
use async_trait::async_trait;

const use_azure: bool = false;

pub async fn get_ada_embedding(text: String) -> Vec<f32> {
    let text = text.replace("\n", " ");
    // TODO: Add potential Azure alternative?
    //if use_azure {
        /*
        openai::Embedding::create(
            &[text],
            &CFG.get_azure_deployment_id_for_model("text-embedding-ada-002"),
        )[0]
            .embedding
            .clone()
        */
   // } else {
        //openai::Embedding::create(&[text], "text-embedding-ada-002")[0]
        //    .embedding
        //    .clone()
    //let open_ai_client = 

    let request = EmbeddingsRequest {
        model: "text-embedding-ada-002".to_string(),
        input: text,
        user: None
    };
    
    let response = open_ai_client.send_request::<EmbeddingsRequest, EmbeddingsResponse>(request).await.unwrap();
    return response.data.embedding;
    //}
}

#[async_trait]
pub trait MemoryProvider {
    async fn add(&mut self, data: String) -> String;
    async fn get(&self, data: String) -> Vec<String>;
    async fn clear(&mut self) -> String;
    async fn get_relevant(&self, data: String, num_relevant: u32) -> Vec<String>;
    async fn get_stats(&self) -> HashMap<String, String>;
}

// Include the function to get Ada embeddings
// It needs to be implemented or replaced with a Rust equivalent