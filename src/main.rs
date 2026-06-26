use anyhow::Result;
use clap::Parser;
use std::io::IsTerminal;

mod cache;
mod cli;
mod config;
mod env_file;
mod history;
mod input;
mod prompt;
mod task;
mod translator;

use cache::{CacheKey, CacheRecord, CacheStore};
use cli::Cli;
use config::{ConfigLoader, ConfigOverrides};
use history::format_recent_queries;
use input::{read_input, validate_input, InputDecision};
use task::TaskType;
use translator::{TranslateRequest, Translator};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.should_show_help_without_input(std::io::stdin().is_terminal()) {
        Cli::print_help()?;
        return Ok(());
    }

    if let Some(recent_request) = cli.recent_request().map_err(anyhow::Error::msg)? {
        show_recent_queries(cli, recent_request).await?;
        return Ok(());
    }

    let raw_input = read_input(cli.input_text(), cli.file.as_deref())?;
    if cli.should_show_help_for_empty_input(&raw_input) {
        Cli::print_help()?;
        return Ok(());
    }

    let input_mode = cli.input_mode();

    match validate_input(&raw_input, input_mode) {
        InputDecision::Translate(text) => run_translation(cli, text).await?,
        InputDecision::Error(message) | InputDecision::Skip(message) => {
            println!("{message}");
        }
    }

    Ok(())
}

async fn show_recent_queries(cli: Cli, recent_request: cli::RecentRequest) -> Result<()> {
    let cache_config = ConfigLoader::load_cache(ConfigOverrides {
        cache_enabled: cli.cache_override(),
        ..ConfigOverrides::default()
    })?;

    let Some(cache_store) = CacheStore::new(cache_config.ttl_days, cache_config.max_bytes) else {
        println!("ERROR: cache directory is not available.");
        return Ok(());
    };

    let queries = cache_store.recent_queries(recent_request.limit, recent_request.filter)?;
    println!("{}", format_recent_queries(recent_request.filter, &queries));
    Ok(())
}

async fn run_translation(cli: Cli, text: &str) -> Result<()> {
    let app_config = ConfigLoader::load(ConfigOverrides {
        provider: cli.provider.clone(),
        model: cli.model.clone(),
        base_url: cli.base_url.clone(),
        target_lang: cli.target.clone(),
        cache_enabled: cli.cache_override(),
    })?;
    let task_type = TaskType::detect(text);
    let cache_key = CacheKey::new(&app_config.model, &app_config.target_lang, task_type, text);
    let cache = if app_config.cache.enabled {
        CacheStore::new(app_config.cache.ttl_days, app_config.cache.max_bytes)
    } else {
        None
    };

    if let Some(cache_store) = &cache {
        if let Some(output) = cache_store.lookup(&cache_key)? {
            println!("{output}");
            return Ok(());
        }
    }

    let translator = Translator::from_config(app_config.model.clone())?;
    let request = TranslateRequest {
        input: text,
        task_type,
        target_lang: &app_config.target_lang,
    };
    let output = if cli.direct {
        let output = translator.translate_direct(&request).await?;
        println!("{}", output.trim());
        output
    } else {
        translator.translate_streaming(&request).await?
    };

    if let Some(cache_store) = &cache {
        cache_store.insert(CacheRecord {
            key: cache_key,
            input: text,
            task_type,
            target_lang: &app_config.target_lang,
            model: &app_config.model,
            output: &output,
        })?;
    }

    Ok(())
}
