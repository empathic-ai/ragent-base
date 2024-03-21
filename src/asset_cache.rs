use std::collections::HashMap;
use std::future::Future;
use bytes::Bytes;
use async_channel;
use tokio::sync::Mutex;
use uuid::Uuid;
use std::sync::Arc;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::Context;
use async_channel::Receiver;

// This type represents the asset you are loading.
#[derive(Clone)]
pub struct Asset {
    pub metadata: HashMap<String, String>,
    pub bytes: Vec<u8>
} // or whatever your asset type is

impl Asset {
    pub fn new(bytes: Vec<u8>) -> Asset {
        Asset {
            metadata: Default::default(),
            bytes: bytes
        }
    }
}

// This enum helps manage assets that are either loaded, or are still loading.
pub enum AssetState {
    Loading(Receiver<Asset>),
    Loaded(Asset),
}

#[derive(Default)]
pub struct AssetCache {
    assets: Arc<Mutex<HashMap<Uuid, AssetState>>>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            assets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get(&self, key: Uuid) -> Result<Asset> {
        let mut assets = self.assets.lock().await;

        match assets.get_mut(&key) {
            Some(AssetState::Loaded(asset)) => Ok(asset.clone()),
            Some(AssetState::Loading(receiver)) => {
                //drop(assets); // Drop the lock so other tasks can access the cache
                let receiver = receiver.clone();
                drop(assets);
                let asset = receiver.recv().await?;
                Ok(asset)
            }
            None => {
                Err(anyhow!("Tried to get an asset that isn't loaded or loading!"))
            }
        }
    }
    
    pub async fn load_asset(&mut self, asset_id: Uuid, load_func: impl Future<Output = Result<Asset>> + Send + 'static, wait_for_completion: bool) -> Result<()> {
        // Your asset loading logic here
        //let mut assets = self.assets.lock().await;
        let (tx, rx) = async_channel::bounded::<Asset>(1);

        self.assets.lock().await.insert(asset_id, AssetState::Loading(rx));
        let _assets = self.assets.clone();
        let load_func = async move {
            let asset = load_func.await.expect("Function failed to load asset");
            _assets.lock().await.insert(asset_id, AssetState::Loaded(asset.clone()));
            tx.send(asset).await;//.expect("Failed to send loaded asset!");
            tx.close();
        };
        if wait_for_completion {
            load_func.await;
        } else {
            tokio::spawn(load_func);
        }
        Ok(())
    }

    /* 
    pub async fn get_or_load(&self, key: Uuid) -> Result<Asset> {
        let mut assets = self.assets.lock().await;

        match assets.get_mut(&key) {
            Some(AssetState::Loaded(asset)) => Ok(asset.clone()),
            Some(AssetState::Loading(receiver)) => {
                //drop(assets); // Drop the lock so other tasks can access the cache
                let asset = receiver.try_recv()?;
                Ok(asset)
            }
            None => {
                let (sender, receiver) = oneshot::channel::<Asset>();

                // Insert the loading state into the cache.
                assets.insert(key, AssetState::Loading(receiver));

                // Drop the lock so other tasks can access the cache.
                drop(assets);

                // Start the loading process (you'll need to implement `load_asset`).
                let asset = load_asset(key).await?;

                // Store the loaded asset in the cache.
                let mut assets = self.assets.lock().await;
                assets.insert(key, AssetState::Loaded(asset.clone()));

                // Notify any other tasks waiting on this asset.
                sender.send(asset.clone());

                Ok(asset)
            }
        }
    }
    */
}

/* 
#[tokio::main]
async fn main() {
    let cache = Arc::new(AssetCache::new());

    let task1 = tokio::spawn(async move {
        let asset = cache.get_or_load("asset1").await.unwrap();
        println!("Task 1 got: {}", asset);
    });

    let task2 = tokio::spawn(async move {
        let asset = cache.get_or_load("asset1").await.unwrap();
        println!("Task 2 got: {}", asset);
    });

    task1.await.unwrap();
    task2.await.unwrap();
}
*/