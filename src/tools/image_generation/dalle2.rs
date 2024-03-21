use std::env;
use anyhow::Result;
use openai_api_rs::v1::{*, api::Client, image::ImageGenerationRequest};
use super::*;

#[derive(Debug, Default)]
pub struct Dalle2 {
    client: reqwest::Client,
}

impl ImageGenerator for Dalle2 {
    async fn get_image(&self, description: String) -> Result<ImageResult> {
        let client = Client::new(env::var("OPENAI_API_KEY").unwrap().to_string());
        let image_response = client.image_generation(ImageGenerationRequest::new(description))?;
        let url = image_response.data[0].clone();

        let bytes = reqwest::get(url.url).await?.bytes().await?;

        Ok(ImageResult { bytes: bytes.to_vec() })
    }
}