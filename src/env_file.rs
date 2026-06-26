use anyhow::{Context, Result};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, Default)]
pub struct EnvSources {
    local: Option<EnvFile>,
    global: Option<EnvFile>,
}

impl EnvSources {
    pub fn load() -> Result<Self> {
        Ok(Self {
            local: EnvFile::load_from_current_dir()?,
            global: EnvFile::load_from_home()?,
        })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.local
            .as_ref()
            .and_then(|env_file| env_file.get(key))
            .or_else(|| self.global.as_ref().and_then(|env_file| env_file.get(key)))
    }

    pub fn has_dotenv_config(&self) -> bool {
        self.local.is_some() || self.global.is_some()
    }

    #[cfg(test)]
    pub(crate) fn from_env_files(local: Option<EnvFile>, global: Option<EnvFile>) -> Self {
        Self { local, global }
    }
}

#[derive(Debug, Clone)]
pub struct EnvFile {
    values: HashMap<String, String>,
}

impl EnvFile {
    pub fn load_from_current_dir() -> Result<Option<Self>> {
        let path = std::env::current_dir()
            .context("failed to resolve current working directory")?
            .join(".env");

        Self::load_optional(path)
    }

    pub fn load_from_home() -> Result<Option<Self>> {
        match home_dir() {
            Some(home) => Self::load_optional(home.join(".translator.env")),
            None => Ok(None),
        }
    }

    fn load_optional(path: PathBuf) -> Result<Option<Self>> {
        if !path.exists() {
            return Ok(None);
        }

        let iter = dotenvy::from_path_iter(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let mut values = HashMap::new();

        for item in iter {
            let (key, value) =
                item.with_context(|| format!("failed to parse {}", path.display()))?;
            values.insert(key.to_ascii_uppercase(), value);
        }

        Ok(Some(Self { values }))
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(&key.to_ascii_uppercase())
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
    }

    #[cfg(test)]
    pub(crate) fn from_pairs(values: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            values: values
                .into_iter()
                .map(|(key, value)| (key.to_ascii_uppercase(), value))
                .collect(),
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn loads_dotenv_file_without_mutating_process_env() {
        let unique_key = format!(
            "TRANSLATOR_ENV_FILE_TEST_ONLY_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let dir = std::env::temp_dir().join(format!(
            "translator-env-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        fs::write(
            &path,
            format!("translator_provider=openai\nTRANSLATOR_MODEL=gpt-4o\n{unique_key}=value\n"),
        )
        .unwrap();

        let env_file = EnvFile::load_optional(path).unwrap().unwrap();

        assert_eq!(env_file.get("TRANSLATOR_PROVIDER"), Some("openai"));
        assert_eq!(env_file.get("translator_model"), Some("gpt-4o"));
        assert_eq!(env_file.get(&unique_key), Some("value"));
        assert!(std::env::var(unique_key).is_err());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn local_env_overrides_global_env() {
        let local = EnvFile::from_pairs([("TRANSLATOR_MODEL".into(), "local-model".into())]);
        let global = EnvFile::from_pairs([("TRANSLATOR_MODEL".into(), "global-model".into())]);
        let sources = EnvSources::from_env_files(Some(local), Some(global));

        assert_eq!(sources.get("TRANSLATOR_MODEL"), Some("local-model"));
        assert!(sources.has_dotenv_config());
    }
}
