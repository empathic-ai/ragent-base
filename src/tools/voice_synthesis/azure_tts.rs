use reqwest::header::{HeaderMap, HeaderValue};
use tokio::sync::Semaphore;
use std::{error::Error, collections::HashMap, env};
use aura_api_helper::*;
use super::*;
use lazy_static::lazy_static;
use async_trait::async_trait;
use anyhow::Result;

lazy_static! {
    pub static ref VOICE_NAME_BY_NAME: HashMap<String, String> = {
        let mut map = HashMap::new();
        map.insert("default".to_string(), "en-US-BrianNeural".to_string());
        map.insert("host".to_string(), "en-US-BrianNeural".to_string());
        map.insert("kind-man-a".to_string(), "en-US-JasonNeural".to_string());
        map.insert("kind-man-b".to_string(), "en-US-SteffanNeural".to_string());
        map.insert("man-a".to_string(), "en-US-TonyNeural".to_string());
        map.insert("young-woman-a".to_string(), "en-US-JaneNeural".to_string());
        map.insert("young-woman-b".to_string(), "en-US-SaraNeural".to_string());
        map.insert("woman-a".to_string(), "en-US-JennyMultilingualNeural".to_string());
        map.insert("woman-b".to_string(), "en-US-AriaNeural".to_string());
        map.insert("woman-c".to_string(), "en-US-NancyNeural".to_string());
        //map.insert("soft-man".to_string(), "".to_string());
        map.insert("deep-man".to_string(), "en-US-en-US-DavisNeural".to_string());
        map.insert("child".to_string(), "en-US-AnaNeural".to_string());
        map
    };
}

#[derive(Debug)]
pub struct AzureSynthesizer {
    pub api_key: String,
    semaphore: Semaphore
}

impl AzureSynthesizer {
    pub fn new_from_env() -> AzureSynthesizer {
        AzureSynthesizer { api_key: env::var("AZURE_API_KEY").unwrap(), semaphore: Semaphore::new(2) }
    }
}

#[async_trait]
impl Synthesizer for AzureSynthesizer {
    /// Refresh the access token. It is recommended to run this command after creating the client
    /// Default sample rate of 24000
    async fn create_speech(&self, emotion: String, voice_name: String, text: String) -> Result<SynthesisResult> {
        let _permit = self.semaphore.acquire().await?;

        let voice_name = VOICE_NAME_BY_NAME.get(&voice_name).unwrap();
        let short_voice_name = get_short_voice_name(voice_name.to_owned());

        let mut emotion = emotion;
        if emotion.trim().is_empty() {
            emotion = "default".to_string();
        }

        //<prosody rate="+10.00%">
        //</prosody>
        let ssml = format!(
r#"<!--ID=B7267351-473F-409D-9765-754A8EBCDE05;Version=1|{{"VoiceNameToIdMapItems":[{{"Id":"520f8b71-e1cc-4e80-b9ea-006d2f816864","Name":"Microsoft Server Speech Text to Speech Voice (en-US, {})","ShortName":"{}","Locale":"en-US","VoiceType":"StandardVoice"}}]}}-->
<!--ID=5B95B1CC-2C7B-494F-B746-CF22A0E779B7;Version=1|null-->
<speak xmlns="http://www.w3.org/2001/10/synthesis" xmlns:mstts="http://www.w3.org/2001/mstts" xmlns:emo="http://www.w3.org/2009/10/emotionml" version="1.0" xml:lang="en-US"><voice name="{}"><mstts:express-as style='{}'>{}</mstts:express-as></voice></speak>"#,
            short_voice_name,
            voice_name,
            voice_name,
            emotion,
            text.clone()
        );

        //print!("BODY: {}", ssml);
    
        let url = "https://eastus.api.cognitive.microsoft.com/sts/v1.0/issueToken";
    
        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/x-www-form-urlencoded"));
        headers.insert("content-length", HeaderValue::from_static("0"));
        headers.insert("Ocp-Apim-Subscription-Key", HeaderValue::from_str(&self.api_key)?);
    
        let client = reqwest::Client::new();
        let response = client.post(url)
            .headers(headers)
            .send()
            .await?;
    
        let x = response.status();//.as_str();
    
        //println!("Headers:\n{:#?}", response.headers());
        let body = response.text().await?;
    
        let url = "https://eastus.tts.speech.microsoft.com/cognitiveservices/v1";
        let authorization = "Bearer ".to_owned() + body.clone().as_str();
    
        let mut headers = HeaderMap::new();
    
        headers.insert("X-Microsoft-OutputFormat", HeaderValue::from_static("riff-24khz-16bit-mono-pcm"));
        headers.insert("content-type", HeaderValue::from_static("application/ssml+xml"));
        headers.insert("Authorization", HeaderValue::from_str(&authorization)?);
        headers.insert("user-agent", HeaderValue::from_static("Meridian"));
    
        let response = client.post(url)
            .headers(headers)
            .body(ssml.clone())
            .send()
            .await?;
    
        //println!("Generated speech: {}", ssml.clone());
        
        let bytes = response.bytes().await?;
    
        Ok(SynthesisResult { bytes: bytes.to_vec() })
        /* 

        let url = "https://e6fz4wogoa.execute-api.us-east-2.amazonaws.com/default/azure-tts";

        let mut headers = HeaderMap::new();
        headers.insert("content-type", HeaderValue::from_static("application/json"));


        let request = SSMLRequest { ssml };

        let client = reqwest::Client::new();
        let response: SSMLResponse = client
            .post(url)
            .headers(headers)
            .json(&request)
            .send()
            .await?
            .json()
            .await?;

        //println!("Status: {}", response.status());
        //let x = response.status();//.as_str();
        //console::info!("Status: ", x.as_str());
        //println!("Headers:\n{:#?}", response.headers());
        //let response: SSMLResponse = response.get::<SSMLResponse>()?;

        //println!("Body:\n{}", body);
        let bytes = base64::decode(response.data)?;

        //console::info!("Body: ", body.clone());

        //console::info!("Generating speech: ", text.clone());
        */


        //gender='Male' name='en-US-ChristopherNeural' - emotionless voice but good Lao Tzu voice
        // en-US-GuyNeural -- very enthusiastic male voice
        //gender='Female' name='en-US-AnaNeural' - really cute, funny young girl voice
        // en-US-JaneNeural -- great Aura voice
        // TonyNeural -- good comedic/sleezy voice

        //console::info!("Generated speech: ", text.clone());

        // create an audio buffer from a given file
        //let file = File::open("samples/sample.wav").unwrap();

        //let bytes = response.bytes().await?;

        /*
                let s: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(7)
                .map(char::from)
                .collect();

                let element = src.dyn_into::<Element>().unwrap();
                element.set_attribute("id", &s);
                let document = web_sys::window().expect("no global `window` exists").document().unwrap();
                document.append_child(&element);
        */

        /*
        let f = async move {
            AudioRecorder::start_playing(bytes.to_vec()).await;
            //browser_sleep(duration).await;
        };
        */

        
    }

    /* TODO: Implement with AWS Lamda functions later
        /// Refresh the access token. It is recommended to run this command after creating the client
        pub async fn text_to_speech(uberduck_id: Uuid, mut text: String, voice_order_id: i32) -> Result<(), Box<dyn Error>> {

            let endpoint_url = format!(
                "https://{api_id}.execute-api.{region}.amazonaws.com/{stage}",
                api_id = api_id,
                region = region,
                stage = stage
            );

            let shared_config = aws_config::from_env().region(region_provider).load().await;

            let api_management_config = config::Builder::from(&shared_config)
                .endpoint_url(endpoint_url)
                .build();

            let client = Client::from_conf(api_management_config);

            client
            .post_to_connection()
            .connection_id(con_id)
            .data(Blob::new(data))
            .send()
            .await?;

            Ok(())
        }
    */
}

fn get_short_voice_name(name: String) -> String {
    let mut name = name;
    if let Some(index) = name.rfind("-") {
        name = name.as_str()[index + 1..].to_string(); // +2 to skip the "::"
    }
    name
}