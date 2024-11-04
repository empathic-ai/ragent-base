use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{Error as IoError, Read, Write};
use std::path::Path;

use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use tokio;
use tokio::sync::Semaphore;
use anyhow::{anyhow, Result};

use async_trait::async_trait;
use super::*;

#[derive(Debug)]
pub struct AzureIdentifier {
    pub api_key: String,
    semaphore: Semaphore
}

impl AzureIdentifier {
    pub fn new_from_env() -> AzureIdentifier {
        AzureIdentifier { api_key: env::var("AZURE_API_KEY").unwrap(), semaphore: Semaphore::new(2) }
    }
}

/*
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example usage:
    let profile_name = "your_profile_name";
    let file_name = "your_file_name";

    create_and_store_profile(profile_name).await?;
    enroll_profile(profile_name).await?;
    let result = verify_profile(file_name, profile_name, None).await?;
    println!("{:#?}", result);

    Ok(())
}*/

#[async_trait]
impl VoiceIdentifier for AzureIdentifier {

    async fn create_voice_profile(&self, profile_name: &str) -> Result<()> {
    //}
    
    //async fn enroll_profile(profile_name: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let speech_key = env::var("Azure_Speech_Key")?;
        let service_region = env::var("Azure_Speech_Region")?;
        let language = env::var("language")?;
        let profile_db = "profiles_db.json";
    
        // Read profile ID from local database or create a new one
        let mut data: HashMap<String, String> = if Path::new(profile_db).exists() {
            let content = fs::read_to_string(profile_db)?;
            serde_json::from_str(&content)?
        } else {
            HashMap::new()
        };
    
        let profile_id = if let Some(id) = data.get(profile_name) {
            id.clone()
        } else {
            let speech_key = env::var("Azure_Speech_Key")?;
            let service_region = env::var("Azure_Speech_Region")?;
            let language = env::var("language")?;
            let profile_db = "profiles_db.json";
        
            // Create Profile URL
            let url = format!(
                "https://{}.api.cognitive.microsoft.com/speaker-recognition/verification/text-independent/profiles?api-version=2021-09-05",
                service_region
            );
        
            // Set headers
            let mut headers = HeaderMap::new();
            headers.insert(
                "Ocp-Apim-Subscription-Key",
                HeaderValue::from_str(&speech_key)?,
            );
            headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        
            // Set body
            let body = serde_json::json!({ "locale": language });
        
            // Send POST request
            let client = reqwest::Client::new();
            let res = client.post(&url).headers(headers).json(&body).send().await?;
        
            // Handle response
            if res.status().is_success() {
                let res_json: Value = res.json().await?;
                let profile_id = res_json["profileId"]
                    .as_str()
                    .ok_or("No profileId in response")?
                    .to_string();
        
                // Read existing profiles or create a new one
                let mut data: HashMap<String, String> = if Path::new(profile_db).exists() {
                    let content = fs::read_to_string(profile_db)?;
                    serde_json::from_str(&content)?
                } else {
                    HashMap::new()
                };
        
                // Store the new profile ID
                data.insert(profile_name.to_string(), profile_id.clone());
                let content = serde_json::to_string(&data)?;
                fs::write(profile_db, content)?;
        
                profile_id
            } else {
                let err_text = res.text().await?;
                return Err(anyhow!(format!("Request failed: {}", err_text).into()));
            }
        };
    
        // Enroll URL
        let url = format!(
            "https://{}.api.cognitive.microsoft.com/speaker-recognition/verification/text-independent/profiles/{}/enrollments?api-version=2021-09-05",
            service_region, profile_id
        );
    
        // Set headers
        let mut headers = HeaderMap::new();
        headers.insert(
            "Ocp-Apim-Subscription-Key",
            HeaderValue::from_str(&speech_key)?,
        );
        headers.insert("Content-Type", HeaderValue::from_static("audio/wav"));
    
        // Read the first audio file
        let file_path = format!("data/{}-{}Neural_first.wav", language, profile_name);
        let audio_data = fs::read(&file_path)?;
    
        // Send the first enrollment request
        let client = reqwest::Client::new();
        let res = client
            .post(&url)
            .headers(headers.clone())
            .body(audio_data)
            .send()
            .await?;
    
        if res.status().is_success() {
            // Read the second audio file
            let file_path = format!("data/{}-{}Neural.wav", language, profile_name);
            let audio_data = fs::read(&file_path)?;
    
            // Send the second enrollment request
            let res = client
                .post(&url)
                .headers(headers)
                .body(audio_data)
                .send()
                .await?;
    
            Ok(res.status().is_success())
        } else {
            let err_text = res.text().await?;
            Err(anyhow!(format!("Enrollment failed: {}", err_text).into()))
        }
    }
    
    async fn identify_voice(&self, voice_names: Vec<String>
        //file_name: &str,
        //profile_name: &str,
        //auth_code: Option<&str>,
    ) -> Result<VoiceIdentificationResponse> {
        let speech_key = env::var("Azure_Speech_Key")?;
        let service_region = env::var("Azure_Speech_Region")?;
        let language = env::var("language")?;
        let profile_db = "profiles_db.json";
    
        // Read profile ID from local database
        let data: HashMap<String, String> = if Path::new(profile_db).exists() {
            let content = fs::read_to_string(profile_db)?;
            serde_json::from_str(&content)?
        } else {
            return Err("Profile database not found".into());
        };
    
        let profile_id = data
            .get(profile_name)
            .ok_or("Profile name not found in database")?;
    
        // Verify URL
        let url = format!(
            "https://{}.api.cognitive.microsoft.com/speaker-recognition/verification/text-independent/profiles/{}:verify?api-version=2021-09-05",
            service_region, profile_id
        );
    
        // Set headers
        let mut headers = HeaderMap::new();
        headers.insert(
            "Ocp-Apim-Subscription-Key",
            HeaderValue::from_str(&speech_key)?,
        );
        headers.insert("Content-Type", HeaderValue::from_static("audio/wav"));
    
        // Read the verification audio file
        let auth_code_str = auth_code.unwrap_or("");
        let file_path = format!(
            "./data/{}-{}Neural_verify{}.wav",
            language, file_name, auth_code_str
        );
        let audio_data = fs::read(&file_path)?;
    
        // Send the verification request
        let client = reqwest::Client::new();
        let res = client
            .post(&url)
            .headers(headers.clone())
            .body(audio_data.clone())
            .send()
            .await?;
    
        let mut result: Value = res.json().await?;
    
        // Speech recognition URL
        let speech_url = format!(
            "https://{}.stt.speech.microsoft.com/speech/recognition/conversation/cognitiveservices/v1?language={}",
            service_region, language
        );
    
        // Send the speech recognition request
        let res = client
            .post(&speech_url)
            .headers(headers)
            .body(audio_data)
            .send()
            .await?;
    
        let speech_result: Value = res.json().await?;
        let display_text = speech_result["DisplayText"]
            .as_str()
            .unwrap_or("")
            .replace('.', "")
            .replace(',', "");
    
        // Add the recognized text to the result
        result["Text"] = Value::String(display_text);
    
        Ok(result)
    }    
}