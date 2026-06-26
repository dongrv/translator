use anyhow::{bail, Context, Result};
use rig::providers::{anthropic, deepseek, openai, zai};

use crate::env_file::EnvSources;

const DEFAULT_TARGET_LANG: &str = "auto";
const DEFAULT_CACHE_TTL_DAYS: u64 = 30;
const DEFAULT_CACHE_MAX_MB: u64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelProvider {
    DeepSeek,
    OpenAi,
    Claude,
    Zhipu,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub model: ModelConfig,
    pub target_lang: String,
    pub cache: CacheConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelConfig {
    pub provider: ModelProvider,
    pub model: String,
    pub api_key: String,
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl_days: u64,
    pub max_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOverrides {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub target_lang: Option<String>,
    pub cache_enabled: Option<bool>,
}

pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load(overrides: ConfigOverrides) -> Result<AppConfig> {
        let sources = EnvSources::load()?;
        AppConfig::from_sources(&sources, overrides)
    }
}

impl AppConfig {
    fn from_sources(sources: &EnvSources, overrides: ConfigOverrides) -> Result<Self> {
        let model = if sources.has_dotenv_config() {
            ModelConfig::from_sources(sources, &overrides)?
        } else {
            ModelConfig::default_deepseek_from_process_env(&overrides)?
        };

        Ok(Self {
            model,
            target_lang: overrides
                .target_lang
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| first_present(sources, &["TRANSLATOR_TARGET_LANG", "TARGET_LANG"]))
                .unwrap_or_else(|| DEFAULT_TARGET_LANG.to_string()),
            cache: CacheConfig::from_sources(sources, &overrides),
        })
    }
}

impl ModelConfig {
    fn from_sources(sources: &EnvSources, overrides: &ConfigOverrides) -> Result<Self> {
        let provider = match overrides.provider.as_deref() {
            Some(provider) => ModelProvider::parse(provider)?,
            None => ModelProvider::parse(
                sources
                    .get("TRANSLATOR_PROVIDER")
                    .or_else(|| sources.get("MODEL_PROVIDER"))
                    .or_else(|| sources.get("LLM_PROVIDER"))
                    .or_else(|| sources.get("PROVIDER"))
                    .unwrap_or("deepseek"),
            )?,
        };

        let model = overrides
            .model
            .clone()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| first_present(sources, model_keys(provider)))
            .unwrap_or_else(|| provider.default_model().to_string());
        let api_key = first_present(sources, api_key_keys(provider))
            .or_else(|| process_env_first(api_key_keys(provider)))
            .with_context(|| format!("missing API key for provider {}", provider.as_str()))?;
        let base_url = overrides
            .base_url
            .clone()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| first_present(sources, base_url_keys(provider)));

        Ok(Self {
            provider,
            model,
            api_key,
            base_url,
        })
    }

    fn default_deepseek_from_process_env(overrides: &ConfigOverrides) -> Result<Self> {
        let provider = overrides
            .provider
            .as_deref()
            .map(ModelProvider::parse)
            .transpose()?
            .unwrap_or(ModelProvider::DeepSeek);
        let api_key = process_env_first(api_key_keys(provider))
            .with_context(|| format!("{} API key is required", provider.as_str()))?;

        Ok(Self {
            provider,
            model: overrides
                .model
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| provider.default_model().to_string()),
            api_key,
            base_url: overrides
                .base_url
                .clone()
                .filter(|value| !value.trim().is_empty()),
        })
    }

    #[cfg(test)]
    fn from_dotenv(env_file: &crate::env_file::EnvFile) -> Result<Self> {
        let sources = EnvSources::from_env_files(Some(env_file.clone()), None);
        Self::from_sources(&sources, &ConfigOverrides::default())
    }
}

impl CacheConfig {
    fn from_sources(sources: &EnvSources, overrides: &ConfigOverrides) -> Self {
        Self {
            enabled: overrides.cache_enabled.unwrap_or_else(|| {
                first_present(sources, &["TRANSLATOR_CACHE", "CACHE"])
                    .map(|value| {
                        !matches!(normalized(&value).as_str(), "0" | "false" | "no" | "off")
                    })
                    .unwrap_or(true)
            }),
            ttl_days: first_present(sources, &["TRANSLATOR_CACHE_TTL_DAYS", "CACHE_TTL_DAYS"])
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_CACHE_TTL_DAYS),
            max_bytes: first_present(sources, &["TRANSLATOR_CACHE_MAX_MB", "CACHE_MAX_MB"])
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(DEFAULT_CACHE_MAX_MB)
                * 1024
                * 1024,
        }
    }
}

impl ModelProvider {
    fn parse(value: &str) -> Result<Self> {
        match normalized(value).as_str() {
            "deepseek" => Ok(Self::DeepSeek),
            "openai" => Ok(Self::OpenAi),
            "claude" | "anthropic" => Ok(Self::Claude),
            "zhipu" | "zhipuai" | "zai" | "glm" | "智普" | "智谱" => Ok(Self::Zhipu),
            other => bail!(
                "unsupported model provider `{other}`; expected deepseek, openai, claude, or zhipu"
            ),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DeepSeek => "deepseek",
            Self::OpenAi => "openai",
            Self::Claude => "claude",
            Self::Zhipu => "zhipu",
        }
    }

    fn default_model(self) -> &'static str {
        match self {
            Self::DeepSeek => deepseek::DEEPSEEK_V4_FLASH,
            Self::OpenAi => openai::GPT_4O_MINI,
            Self::Claude => anthropic::completion::CLAUDE_HAIKU_4_5,
            Self::Zhipu => zai::GLM_4_6_AIR,
        }
    }
}

fn first_present(sources: &EnvSources, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| sources.get(key))
        .map(str::to_owned)
}

fn process_env_first(keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| std::env::var(key).ok())
        .filter(|value| !value.trim().is_empty())
}

fn model_keys(provider: ModelProvider) -> &'static [&'static str] {
    match provider {
        ModelProvider::DeepSeek => &["TRANSLATOR_MODEL", "MODEL", "DEEPSEEK_MODEL"],
        ModelProvider::OpenAi => &["TRANSLATOR_MODEL", "MODEL", "OPENAI_MODEL"],
        ModelProvider::Claude => &[
            "TRANSLATOR_MODEL",
            "MODEL",
            "CLAUDE_MODEL",
            "ANTHROPIC_MODEL",
        ],
        ModelProvider::Zhipu => &[
            "TRANSLATOR_MODEL",
            "MODEL",
            "ZHIPU_MODEL",
            "ZHIPUAI_MODEL",
            "ZAI_MODEL",
        ],
    }
}

fn api_key_keys(provider: ModelProvider) -> &'static [&'static str] {
    match provider {
        ModelProvider::DeepSeek => &["TRANSLATOR_API_KEY", "API_KEY", "DEEPSEEK_API_KEY"],
        ModelProvider::OpenAi => &["TRANSLATOR_API_KEY", "API_KEY", "OPENAI_API_KEY"],
        ModelProvider::Claude => &[
            "TRANSLATOR_API_KEY",
            "API_KEY",
            "CLAUDE_API_KEY",
            "ANTHROPIC_API_KEY",
        ],
        ModelProvider::Zhipu => &[
            "TRANSLATOR_API_KEY",
            "API_KEY",
            "ZHIPU_API_KEY",
            "ZHIPUAI_API_KEY",
            "ZAI_API_KEY",
        ],
    }
}

fn base_url_keys(provider: ModelProvider) -> &'static [&'static str] {
    match provider {
        ModelProvider::DeepSeek => &[
            "TRANSLATOR_BASE_URL",
            "BASE_URL",
            "DEEPSEEK_BASE_URL",
            "DEEPSEEK_API_BASE",
            "DEEPSEEK_API_BASE_URL",
        ],
        ModelProvider::OpenAi => &[
            "TRANSLATOR_BASE_URL",
            "BASE_URL",
            "OPENAI_BASE_URL",
            "OPENAI_API_BASE",
            "OPENAI_API_BASE_URL",
        ],
        ModelProvider::Claude => &[
            "TRANSLATOR_BASE_URL",
            "BASE_URL",
            "CLAUDE_BASE_URL",
            "CLAUDE_API_BASE",
            "CLAUDE_API_BASE_URL",
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_API_BASE",
            "ANTHROPIC_API_BASE_URL",
        ],
        ModelProvider::Zhipu => &[
            "TRANSLATOR_BASE_URL",
            "BASE_URL",
            "ZHIPU_BASE_URL",
            "ZHIPU_API_BASE",
            "ZHIPU_API_BASE_URL",
            "ZHIPUAI_BASE_URL",
            "ZAI_BASE_URL",
            "ZAI_API_BASE",
        ],
    }
}

fn normalized(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_file::EnvFile;

    fn env_file(contents: &[(&str, &str)]) -> EnvFile {
        EnvFile::from_pairs(
            contents
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string())),
        )
    }

    fn sources(contents: &[(&str, &str)]) -> EnvSources {
        EnvSources::from_env_files(Some(env_file(contents)), None)
    }

    #[test]
    fn parses_openai_dotenv_config() {
        let env_file = env_file(&[
            ("TRANSLATOR_PROVIDER", "openai"),
            ("OPENAI_API_KEY", "key"),
            ("OPENAI_MODEL", "gpt-4o"),
            ("OPENAI_BASE_URL", "https://proxy.example.com/v1"),
        ]);

        let config = ModelConfig::from_dotenv(&env_file).unwrap();

        assert_eq!(config.provider, ModelProvider::OpenAi);
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.api_key, "key");
        assert_eq!(
            config.base_url.as_deref(),
            Some("https://proxy.example.com/v1")
        );
    }

    #[test]
    fn defaults_to_deepseek_flash_when_dotenv_omits_provider_and_model() {
        let env_file = env_file(&[("DEEPSEEK_API_KEY", "key")]);

        let config = ModelConfig::from_dotenv(&env_file).unwrap();

        assert_eq!(config.provider, ModelProvider::DeepSeek);
        assert_eq!(config.model, deepseek::DEEPSEEK_V4_FLASH);
        assert_eq!(config.api_key, "key");
    }

    #[test]
    fn accepts_zhipu_aliases() {
        let env_file = env_file(&[("TRANSLATOR_PROVIDER", "zhipu"), ("ZHIPU_API_KEY", "key")]);
        let config = ModelConfig::from_dotenv(&env_file).unwrap();

        assert_eq!(config.provider, ModelProvider::Zhipu);
        assert_eq!(config.model, zai::GLM_4_6_AIR);
    }

    #[test]
    fn reads_target_language_and_cache_settings() {
        let config = AppConfig::from_sources(
            &sources(&[
                ("DEEPSEEK_API_KEY", "key"),
                ("TARGET_LANG", "English"),
                ("TRANSLATOR_CACHE_TTL_DAYS", "7"),
                ("TRANSLATOR_CACHE_MAX_MB", "2"),
            ]),
            ConfigOverrides::default(),
        )
        .unwrap();

        assert_eq!(config.target_lang, "English");
        assert_eq!(config.cache.ttl_days, 7);
        assert_eq!(config.cache.max_bytes, 2 * 1024 * 1024);
    }

    #[test]
    fn cli_overrides_env_settings() {
        let config = AppConfig::from_sources(
            &sources(&[
                ("DEEPSEEK_API_KEY", "key"),
                ("TARGET_LANG", "Chinese"),
                ("TRANSLATOR_CACHE", "true"),
            ]),
            ConfigOverrides {
                model: Some("custom-model".into()),
                target_lang: Some("Japanese".into()),
                cache_enabled: Some(false),
                ..ConfigOverrides::default()
            },
        )
        .unwrap();

        assert_eq!(config.model.model, "custom-model");
        assert_eq!(config.target_lang, "Japanese");
        assert!(!config.cache.enabled);
    }
}
