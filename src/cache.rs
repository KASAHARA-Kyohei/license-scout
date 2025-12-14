use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::types::PackageMetadata;

#[derive(Debug)]
pub struct LicenseCache {
    path: PathBuf,
    data: CacheData,
    dirty: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheData {
    version: u8,
    entries: HashMap<String, PackageMetadata>,
}

impl Default for CacheData {
    fn default() -> Self {
        Self {
            version: 1,
            entries: HashMap::new(),
        }
    }
}

impl LicenseCache {
    pub fn load() -> Result<Self> {
        let path = default_cache_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("キャッシュディレクトリの作成に失敗: {}", parent.display())
            })?;
        }

        let data = if path.exists() {
            let content = fs::read_to_string(&path).with_context(|| {
                format!("キャッシュファイルの読み込みに失敗: {}", path.display())
            })?;
            serde_json::from_str(&content)
                .with_context(|| format!("キャッシュファイルの解析に失敗: {}", path.display()))?
        } else {
            CacheData::default()
        };

        Ok(Self {
            path,
            data,
            dirty: false,
        })
    }

    pub fn get(&self, manager: &str, name: &str) -> Option<PackageMetadata> {
        let key = cache_key(manager, name);
        self.data.entries.get(&key).cloned()
    }

    pub fn insert(&mut self, manager: &str, name: &str, metadata: PackageMetadata) {
        let key = cache_key(manager, name);
        self.data.entries.insert(key, metadata);
        self.dirty = true;
    }

    pub fn save(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        let json =
            serde_json::to_string_pretty(&self.data).context("キャッシュのJSON化に失敗しました")?;
        fs::write(&self.path, json).with_context(|| {
            format!(
                "キャッシュファイルの書き込みに失敗: {}",
                self.path.display()
            )
        })?;
        self.dirty = false;
        Ok(())
    }
}

fn cache_key(manager: &str, name: &str) -> String {
    format!(
        "{}::{}",
        manager.to_ascii_lowercase(),
        name.to_ascii_lowercase()
    )
}

fn default_cache_path() -> PathBuf {
    if let Some(dir) = dirs::cache_dir() {
        dir.join("license-scout").join("license-cache.json")
    } else {
        Path::new(".license-scout-cache.json").to_path_buf()
    }
}
