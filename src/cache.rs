use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
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

#[derive(Debug, Clone)]
pub struct CacheRecord<'a> {
    pub key: CacheKey,
    pub input: &'a str,
    pub task_type: TaskType,
    pub target_lang: &'a str,
    pub model: &'a ModelConfig,
    pub output: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentQuery {
    pub input: String,
    pub task_type: TaskType,
    pub target_lang: String,
    pub provider: String,
    pub model: String,
    pub output: String,
    pub created_at: u64,
    pub last_used_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecentFilter {
    All,
    Words,
    Sentences,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    key: String,
    input: String,
    task_type: String,
    target_lang: String,
    provider: String,
    model: String,
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

    pub fn insert(&self, record: CacheRecord<'_>) -> Result<()> {
        if record.output.trim().is_empty() {
            return Ok(());
        }

        let now = now_unix();
        let mut entries = self.load_entries()?;
        entries.retain(|entry| self.is_fresh(entry, now) && entry.key != record.key.0);
        entries.push(CacheEntry {
            key: record.key.0,
            input: normalized_input(record.input),
            task_type: record.task_type.cache_label().to_string(),
            target_lang: record.target_lang.trim().to_string(),
            provider: record.model.provider.as_str().to_string(),
            model: record.model.model.clone(),
            output: record.output.trim().to_string(),
            created_at: now,
            last_used_at: now,
        });

        self.write_entries(&mut entries)
    }

    pub fn recent_queries(&self, limit: usize, filter: RecentFilter) -> Result<Vec<RecentQuery>> {
        if limit == 0 {
            return Ok(Vec::new());
        }

        let now = now_unix();
        let mut entries = self.load_entries()?;
        let original_len = entries.len();
        entries.retain(|entry| self.is_fresh(entry, now));

        if entries.len() != original_len {
            self.write_entries(&mut entries)?;
        }

        let mut indexed_entries = entries.into_iter().enumerate().collect::<Vec<_>>();
        indexed_entries.sort_by(|(left_index, left), (right_index, right)| {
            right
                .last_used_at
                .cmp(&left.last_used_at)
                .then_with(|| right_index.cmp(left_index))
        });

        let mut queries = Vec::new();
        let mut seen = HashSet::new();
        for (_, entry) in indexed_entries {
            let Some(task_type) = TaskType::from_cache_label(&entry.task_type) else {
                continue;
            };

            if !matches_filter(task_type, filter) {
                continue;
            }

            let history_key = history_key(task_type, &entry.target_lang, &entry.input);
            if !seen.insert(history_key) {
                continue;
            }

            queries.push(RecentQuery {
                input: entry.input,
                task_type,
                target_lang: entry.target_lang,
                provider: entry.provider,
                model: entry.model,
                output: entry.output,
                created_at: entry.created_at,
                last_used_at: entry.last_used_at,
            });

            if queries.len() == limit {
                break;
            }
        }

        Ok(queries)
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

fn matches_filter(task_type: TaskType, filter: RecentFilter) -> bool {
    match filter {
        RecentFilter::All => true,
        RecentFilter::Words => task_type == TaskType::WordLookup,
        RecentFilter::Sentences => task_type == TaskType::SentenceTranslation,
    }
}

fn normalized_input(input: &str) -> String {
    input
        .trim()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn history_key(task_type: TaskType, target_lang: &str, input: &str) -> String {
    format!(
        "{}\0{}\0{}",
        task_type.cache_label(),
        target_lang.trim().to_ascii_lowercase(),
        normalized_input(input).to_ascii_lowercase()
    )
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

    fn record<'a>(
        model: &'a ModelConfig,
        input: &'a str,
        task_type: TaskType,
        output: &'a str,
    ) -> CacheRecord<'a> {
        CacheRecord {
            key: CacheKey::new(model, "auto", task_type, input),
            input,
            task_type,
            target_lang: "auto",
            model,
            output,
        }
    }

    fn store(name: &str, max_bytes: u64) -> (PathBuf, CacheStore) {
        let dir = std::env::temp_dir().join(format!("translator-cache-test-{name}-{}", now_unix()));
        let store = CacheStore {
            path: dir.join("cache.jsonl"),
            ttl_seconds: 30 * 24 * 60 * 60,
            max_bytes,
        };
        (dir, store)
    }

    #[test]
    fn cache_key_changes_with_model() {
        let a = CacheKey::new(&model("a"), "auto", TaskType::SentenceTranslation, "hello");
        let b = CacheKey::new(&model("b"), "auto", TaskType::SentenceTranslation, "hello");

        assert_ne!(a, b);
    }

    #[test]
    fn insert_replaces_duplicate_query_record() {
        let (dir, store) = store("replace", 10_000);
        let model = model("m");

        store
            .insert(record(
                &model,
                "hello",
                TaskType::SentenceTranslation,
                "你好",
            ))
            .unwrap();
        store
            .insert(record(
                &model,
                "hello",
                TaskType::SentenceTranslation,
                "您好",
            ))
            .unwrap();

        let queries = store.recent_queries(10, RecentFilter::All).unwrap();
        assert_eq!(queries.len(), 1);
        assert_eq!(queries[0].output, "您好");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn recent_queries_filter_and_sort_by_last_used() {
        let (dir, store) = store("recent", 10_000);
        let model = model("m");

        store
            .insert(record(&model, "burst", TaskType::WordLookup, "WORD: burst"))
            .unwrap();
        store
            .insert(record(
                &model,
                "hello world",
                TaskType::SentenceTranslation,
                "你好，世界",
            ))
            .unwrap();

        let all = store.recent_queries(10, RecentFilter::All).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].input, "hello world");

        let words = store.recent_queries(10, RecentFilter::Words).unwrap();
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].task_type, TaskType::WordLookup);

        let sentences = store.recent_queries(10, RecentFilter::Sentences).unwrap();
        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0].task_type, TaskType::SentenceTranslation);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn recent_queries_deduplicate_by_input_target_and_task() {
        let (dir, store) = store("dedupe", 10_000);
        let model_a = model("a");
        let model_b = model("b");

        store
            .insert(record(
                &model_a,
                "hello",
                TaskType::SentenceTranslation,
                "你好",
            ))
            .unwrap();
        store
            .insert(record(
                &model_b,
                "hello",
                TaskType::SentenceTranslation,
                "您好",
            ))
            .unwrap();

        let all = store.recent_queries(10, RecentFilter::All).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].model, "b");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn prunes_oldest_entry_when_cache_is_too_large() {
        let (dir, store) = store("prune", 260);
        let model = model("m");

        store
            .insert(record(
                &model,
                "one",
                TaskType::SentenceTranslation,
                "first output",
            ))
            .unwrap();
        store
            .insert(record(
                &model,
                "two",
                TaskType::SentenceTranslation,
                "second output with a little more text",
            ))
            .unwrap();

        assert!(store.load_entries().unwrap().len() <= 1);
        let _ = fs::remove_dir_all(dir);
    }
}
