//extern crate pinecone;
//extern crate colorama;
use std::collections::HashMap;
use std::hash::Hash;
use client_sdk::{client::pinecone_client::PineconeClient, data_types::Vector};
use client_sdk::utils::errors::PineconeClientError;
use client_sdk::data_types::Db;
use client_sdk::index::Index;
//use pinecone::{init, whoami, list_indexes, create_index, Index};
//use pinecone::{init, whoami, list_indexes, create_index, Index};
//use colorama::{Fore, Style};
//use autogpt::logger::logger;
use super::base::{MemoryProvider, get_ada_embedding};
use async_trait::async_trait;
use std::collections::BTreeMap;
use client_sdk::data_types::MetadataValue;

pub struct PineconeMemory {
    index: Index,
    vec_num: u32,
}

impl PineconeMemory {
    pub async fn new(pinecone_api_key: String, pinecone_region: String, project_id: String) -> Result<PineconeMemory, PineconeClientError>  {
        let client = PineconeClient::new(Some(&pinecone_api_key), Some(&pinecone_region), Some(&project_id)).await?;

        let dimension = 1536;
        let metric = "cosine".to_string();
        let pod_type = "p1".to_string();
        let table_name = "auto-gpt".to_string();
        let vec_num = 0;

        /*
        let response = context.client.whoami().await;
        match whoami() {
            Ok(_) => (),
            Err(e) => {
                panic!("{}", e);
                logger::typewriter_log(
                    "FAILED TO CONNECT TO PINECONE",
                    Fore::RED,
                    format!("{}{}{}", Style::BRIGHT, e, Style::RESET_ALL),
                );
                logger::double_check(
                    format!("Please ensure you have setup and configured Pinecone properly for use. \
                    You can check out {}https://github.com/Torantulino/Auto-GPT#-pinecone-api-key-setup{} \
                    to ensure you've set up everything correctly.",
                    format!("{}{}{}", Fore::CYAN, Style::BRIGHT, Style::RESET_ALL))
                );
                std::process::exit(1);
            }
        }
        */
        
        if !client.list_indexes().await?.contains(&table_name.to_string()) {
            let db = Db {
                name: table_name.clone(),
                dimension: dimension,
                metric: Some(metric),
                pod_type: Some(pod_type),
                ..Default::default()
            };
            client.create_index(db, None, None).await?;
        }

        let index = client.get_index(&table_name).await?;

        Ok(PineconeMemory { index, vec_num })
    }
}

#[async_trait]
impl MemoryProvider for PineconeMemory {
    async fn add(&mut self, data: String) -> String {
        
        let vector = get_ada_embedding(data.clone()).await;

        let mut metadata: BTreeMap<String, MetadataValue> = BTreeMap::new();

        // Insert a value into the BTreeMap
        metadata.insert(
            "raw_text".to_string(),
            MetadataValue::StringVal(data.clone())
        );
    
        let v = Vector{id: self.vec_num.to_string(), values: vector, sparse_values: None, metadata: Some(metadata) };
        let _v = &[v];
        let resp = self.index.upsert("", _v, None);
        // let resp = self.index.upsert(vec![(self.vec_num.to_string(), vector, [("raw_text", data)])]);

        let _text = format!("Inserting data into memory at index: {}:\n data: {}", self.vec_num, data);
        self.vec_num += 1;
        _text
    }

    async fn get(&self, data: String) -> Vec<String> {
        self.get_relevant(data, 1).await
    }

    async fn clear(&mut self) -> String {
        todo!()
        //self.index.delete(true);//delete_all: true);
        //"Obliviated".to_string()
    }

    async fn get_relevant(&self, data: String, num_relevant: u32) -> Vec<String> {
        let query_embedding = get_ada_embedding(data).await;
        let mut results = self.index.to_owned().query("", Some(query_embedding), None, num_relevant, None, false, true).await.unwrap(); //include_metadata: 
        results.sort_by(|x, y| x.score.partial_cmp(&y.score).unwrap());
        //let sorted_results: Vec<_> = results.sort_by(|x, y| x.score.partial_cmp(&y.score).unwrap());//.collect();
        results.iter().map(|item| {
            let raw_text = &item.to_owned().metadata.as_ref().unwrap()["raw_text"];
            match raw_text {
                MetadataValue::StringVal(s) => s.clone(),
                _ => "The metadata_value is not a StringVal variant.".to_string(),
            }
        }).collect()
    }

    async fn get_stats(&self) -> HashMap<String, String> {
        let x = self.index.to_owned().describe_index_stats(None).await.unwrap();

        let mut stats = HashMap::<String, String>::new();
        stats.insert("dimension".to_string(), x.dimension.to_string());
        stats.insert("index_fullness".to_string(), x.index_fullness.to_string());
        stats.insert("total_vector_count".to_string(), x.total_vector_count.to_string());

        stats
    }
}