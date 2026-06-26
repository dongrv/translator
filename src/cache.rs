use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{config::ModelConfig, task::TaskType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey(String);

#[derive(Debug, Clone)]
pub struct CacheStore {
    path: PathBuf,
    ttl_seconds: u64,
    max_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    key: String,
    output: String,
    created_at: u64,
    last_used_at: u64,
}

impl CacheKey {
    pub fn new(model: &ModelConfig, target_lang: &str, task_type: TaskType, input: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(model.provider.as_str());
        hasher.update(b"\0");
        hasher.update(model.model.as_bytes());
        hasher.update(b"\0");
        hasher.update(target_lang.as_bytes());
        hasher.update(b"\0");
        hasher.update(task_type.cache_label().as_bytes());
        hasher.update(b"\0");
        hasher.update(input.as_bytes());

        Self(format!("{:x}", hasher.finalize()))
    }
}

impl CacheStore {
    pub fn new(ttl_days: u64, max_bytes: u64) -> Option<Self> {
        cache_path().map(|path| Self {
            path,
            ttl_seconds: ttl_days.saturating_mul(24 * 60 * 60),
            max_bytes,
        })
    }

    pub fn lookup(&self, key: &CacheKey) -> Result<Option<String>> {
        let now = now_unix();
        let mut entries = self.load_entries()?;
        let mut hit = None;
        let mut changed = false;

        entries.retain(|entry| {
            let fresh = self.is_fresh(entry, now);
            changed |= !fresh;
            fresh
        });

        for entry in &mut entries {
            if entry.key == key.0 {
                entry.last_used_at = now;
                hit = Some(entry.output.clone());
                changed = true;
                break;
            }
        }

        if changed {
            self.write_entries(&mut entries)?;
        }

        Ok(hit)
    }

    pub fn insert(&self, key: CacheKey, output: &str) -> Result<()> {
        if output.trim().is_empty() {
            return Ok(());
        }

        let now = now_unix();
        let mut entries = self.load_entries()?;
        entries.retain(|entry| self.is_fresh(entry, now) && entry.key != key.0);
        entries.push(CacheEntry {
            key: key.0,
            output: output.trim().to_string(),
            created_at: now,
            last_used_at: now,
        });

        self.write_entries(&mut entries)
    }

    fn load_entries(&self) -> Result<Vec<CacheEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let contents = fs::read_to_string(&self.path)
            .with_context(|| format!("failed to read cache {}", self.path.display()))?;
        let mut entries = Vec::new();

        for line in contents.lines().filter(|line| !line.trim().is_empty()) {
            if let Ok(entry) = serde_json::from_str::<CacheEntry>(line) {
                entries.push(entry);
            }
        }

        Ok(entries)
    }

    fn write_entries(&self, entries: &mut Vec<CacheEntry>) -> Result<()> {
        self.prune_to_size(entries)?;

        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create cache dir {}", parent.display()))?;
        }

        fs::write(&self.path, serialize_entries(entries)?)
            .with_context(|| format!("failed to write cache {}", self.path.display()))
    }

    fn prune_to_size(&self, entries: &mut Vec<CacheEntry>) -> Result<()> {
        while serialized_len(entries)? > self.max_bytes as usize && !entries.is_empty() {
            if let Some(index) = entries
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| entry.last_used_at)
                .map(|(index, _)| index)
            {
                entries.remove(index);
            } else {
                break;
            }
        }

        Ok(())
    }

    fn is_fresh(&self, entry: &CacheEntry, now: u64) -> bool {
        now.saturating_sub(entry.created_at) <= self.ttl_seconds
    }
}

fn cache_path() -> Option<PathBuf> {
    std::env::var_os("TRANSLATOR_CACHE_PATH")
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|home| home.join(".translator").join("cache.jsonl")))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn serialized_len(entries: &[CacheEntry]) -> Result<usize> {
    Ok(serialize_entries(entries)?.len())
}

fn serialize_entries(entries: &[CacheEntry]) -> Result<String> {
    let mut lines = String::new();
    for entry in entries {
        lines.push_str(&serde_json::to_string(entry)?);
        lines.push('\n');
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModelProvider;

    fn model(name: &str) -> ModelConfig {
        ModelConfig {
            provider: ModelProvider::DeepSeek,
            model: name.to_string(),
            api_key: "key".to_string(),
            base_url: None,
        }
    }

    #[test]
    fn cache_key_changes_with_model() {
        let a = CacheKey::new(&model("a"), "auto", TaskType::SentenceTranslation, "hello");
        let b = CacheKey::new(&model("b"), "auto", TaskType::SentenceTranslation, "hello");

        assert_ne!(a, b);
    }

    #[test]
    fn prunes_oldest_entry_when_cache_is_too_large() {
        let dir = std::env::temp_dir().join(format!("translator-cache-test-{}", now_unix()));
        let store = CacheStore {
            path: dir.join("cache.jsonl"),
            ttl_seconds: 30 * 24 * 60 * 60,
            max_bytes: 180,
        };

        store
            .insert(
                CacheKey::new(&model("m"), "auto", TaskType::SentenceTranslation, "one"),
                "first output",
            )
            .unwrap();
        store
            .insert(
                CacheKey::new(&model("m"), "auto", TaskType::SentenceTranslation, "two"),
                "second output with a little more text",
            )
            .unwrap();

        assert!(store.load_entries().unwrap().len() <= 1);
        let _ = fs::remove_dir_all(dir);
    }
}
