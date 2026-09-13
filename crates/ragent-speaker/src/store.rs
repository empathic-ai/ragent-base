use crate::{VoiceProfile, VoiceTemplate, unit_vector};
use anyhow::{Result, ensure};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, path::PathBuf};
use tokio::io::AsyncWriteExt;

pub const MAX_PROFILES: usize = 128;
pub fn validate_profiles(profiles: &[VoiceProfile]) -> Result<()> {
    ensure!(profiles.len() <= MAX_PROFILES, "too many voice profiles");
    let mut seen = HashSet::new();
    for profile in profiles {
        ensure!(
            !profile.user_id.is_empty()
                && profile.user_id.len() <= 256
                && !profile.model_id.is_empty()
                && profile.model_id.len() <= 256,
            "invalid profile id"
        );
        ensure!(
            seen.insert((&profile.user_id, &profile.model_id)),
            "duplicate profile"
        );
        match &profile.template {
            VoiceTemplate::Embedding(v) => {
                unit_vector(v)?;
            }
            VoiceTemplate::Opaque(v) => ensure!(
                !v.is_empty() && v.len() <= 1024 * 1024,
                "invalid voiceprint"
            ),
        }
    }
    Ok(())
}
/// The host chooses a tenant/space scoped store. Do not share an unrestricted
/// account-wide gallery between unrelated microphone sessions.
#[async_trait]
pub trait VoiceProfileStore: Send + Sync {
    async fn load(&self) -> Result<Vec<VoiceProfile>>;
    /// Atomically replace this scope's snapshot; use one writer per scope.
    async fn save(&self, profiles: &[VoiceProfile]) -> Result<()>;
}

/// Local deployment store: private directory and atomic 0600 snapshots on Unix.
/// For multiple processes use a database implementation with transaction control.
pub struct FileProfileStore {
    path: PathBuf,
}
impl FileProfileStore {
    pub fn new(directory: impl Into<PathBuf>, scope: &str) -> Self {
        let key = format!("{:x}", Sha256::digest(scope.as_bytes()));
        Self {
            path: directory.into().join(format!("{key}.json")),
        }
    }
}
#[async_trait]
impl VoiceProfileStore for FileProfileStore {
    async fn load(&self) -> Result<Vec<VoiceProfile>> {
        if let Ok(meta) = tokio::fs::metadata(&self.path).await {
            ensure!(
                meta.len() <= 16 * 1024 * 1024,
                "profile store exceeds size limit"
            );
        }
        let bytes = match tokio::fs::read(&self.path).await {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.into()),
        };
        ensure!(
            bytes.len() <= 16 * 1024 * 1024,
            "profile store exceeds size limit"
        );
        let profiles: Vec<VoiceProfile> = serde_json::from_slice(&bytes)?;
        validate_profiles(&profiles)?;
        Ok(profiles)
    }
    async fn save(&self, profiles: &[VoiceProfile]) -> Result<()> {
        validate_profiles(profiles)?;
        let parent = self.path.parent().expect("profile path has parent");
        tokio::fs::create_dir_all(parent).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).await?;
        }
        let temporary = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = async {
            let mut options = tokio::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options.open(&temporary).await?;
            let bytes = serde_json::to_vec(profiles)?;
            ensure!(
                bytes.len() <= 16 * 1024 * 1024,
                "profile store exceeds size limit"
            );
            file.write_all(&bytes).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&temporary, &self.path).await?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result
    }
}
