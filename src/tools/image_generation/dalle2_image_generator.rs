use std::env;
use anyhow::Result;
use openai_api_rs::v1::{api::{OpenAIClient, OpenAIClientBuilder}, image::ImageGenerationRequest, *};
use super::*;

#[derive(Debug, Default)]
pub struct Dalle2ImageGenerator {
    client: reqwest::Client,
}

impl ImageGenerator for Dalle2ImageGenerator {
    async fn get_image(&self, description: String) -> Result<ImageResult> {
        let client = OpenAIClientBuilder::new().with_api_key(env::var("OPENAI_API_KEY").unwrap().to_string()).build().unwrap();
        let image_response = client.image_generation(ImageGenerationRequest::new(description)).await?;
        let url = &image_response.inner.data[0];

        let bytes = reqwest::get(url.url.clone()).await?.bytes().await?;

        Ok(ImageResult { bytes: bytes.to_vec() })
    }
}