use anyhow::Result;
use futures::StreamExt;
use rig::client::CompletionClient;
use rig::completion::Prompt;
use rig::providers::{anthropic, deepseek, openai, zai};
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use std::io::{self, Write};

use crate::{
    config::{ModelConfig, ModelProvider},
    prompt::{build_user_prompt, SYSTEM_PROMPT},
    task::TaskType,
};

pub struct TranslateRequest<'a> {
    pub input: &'a str,
    pub task_type: TaskType,
    pub target_lang: &'a str,
}

impl TranslateRequest<'_> {
    fn to_prompt(&self) -> String {
        build_user_prompt(self.input, self.task_type, self.target_lang)
    }
}

pub struct Translator {
    backend: TranslatorBackend,
}

enum TranslatorBackend {
    DeepSeek(rig::agent::Agent<deepseek::CompletionModel>),
    OpenAi(rig::agent::Agent<openai::completion::CompletionModel>),
    Claude(rig::agent::Agent<anthropic::completion::CompletionModel>),
    Zhipu(rig::agent::Agent<openai::completion::GenericCompletionModel<zai::ZAiExt>>),
}

impl Translator {
    pub fn from_config(config: ModelConfig) -> Result<Self> {
        let backend = match config.provider {
            ModelProvider::DeepSeek => {
                let mut builder = deepseek::Client::builder().api_key(config.api_key);
                if let Some(base_url) = config.base_url {
                    builder = builder.base_url(base_url);
                }
                TranslatorBackend::DeepSeek(build_agent(builder.build()?, config.model))
            }
            ModelProvider::OpenAi => {
                let mut builder = openai::CompletionsClient::builder().api_key(config.api_key);
                if let Some(base_url) = config.base_url {
                    builder = builder.base_url(base_url);
                }
                TranslatorBackend::OpenAi(build_agent(builder.build()?, config.model))
            }
            ModelProvider::Claude => {
                let mut builder = anthropic::Client::builder().api_key(config.api_key);
                if let Some(base_url) = config.base_url {
                    builder = builder.base_url(base_url);
                }
                TranslatorBackend::Claude(build_agent(builder.build()?, config.model))
            }
            ModelProvider::Zhipu => {
                let mut builder = zai::Client::builder().api_key(config.api_key);
                if let Some(base_url) = config.base_url {
                    builder = builder.base_url(base_url);
                }
                TranslatorBackend::Zhipu(build_agent(builder.build()?, config.model))
            }
        };

        Ok(Self { backend })
    }

    pub async fn translate_direct(&self, request: &TranslateRequest<'_>) -> Result<String> {
        match self.try_translate_direct(request).await {
            Ok(output) => Ok(output),
            Err(first_error) => {
                eprintln!("WARN: request failed, retrying once...");
                self.try_translate_direct(request)
                    .await
                    .map_err(|second_error| {
                        anyhow::anyhow!(
                            "ERROR: request failed after retry: {second_error}; first error: {first_error}"
                        )
                    })
            }
        }
    }

    pub async fn translate_streaming(&self, request: &TranslateRequest<'_>) -> Result<String> {
        match self.try_translate_streaming(request).await {
            Ok(output) => Ok(output),
            Err(first_error) => {
                eprintln!("WARN: request failed, retrying once...");
                self.try_translate_streaming(request)
                    .await
                    .map_err(|second_error| {
                        anyhow::anyhow!(
                            "ERROR: request failed after retry: {second_error}; first error: {first_error}"
                        )
                    })
            }
        }
    }

    async fn try_translate_direct(&self, request: &TranslateRequest<'_>) -> Result<String> {
        let prompt = request.to_prompt();
        match &self.backend {
            TranslatorBackend::DeepSeek(agent) => Ok(agent.prompt(prompt).await?),
            TranslatorBackend::OpenAi(agent) => Ok(agent.prompt(prompt).await?),
            TranslatorBackend::Claude(agent) => Ok(agent.prompt(prompt).await?),
            TranslatorBackend::Zhipu(agent) => Ok(agent.prompt(prompt).await?),
        }
    }

    async fn try_translate_streaming(&self, request: &TranslateRequest<'_>) -> Result<String> {
        let prompt = request.to_prompt();
        match &self.backend {
            TranslatorBackend::DeepSeek(agent) => stream_agent(agent, &prompt).await,
            TranslatorBackend::OpenAi(agent) => stream_agent(agent, &prompt).await,
            TranslatorBackend::Claude(agent) => stream_agent(agent, &prompt).await,
            TranslatorBackend::Zhipu(agent) => stream_agent(agent, &prompt).await,
        }
    }
}

fn build_agent<C>(client: C, model: String) -> rig::agent::Agent<C::CompletionModel>
where
    C: CompletionClient,
    C::CompletionModel: rig::completion::CompletionModel + 'static,
{
    client
        .agent(model)
        .preamble(SYSTEM_PROMPT)
        .temperature(0.2)
        .build()
}

async fn stream_agent<M>(agent: &rig::agent::Agent<M>, input: &str) -> Result<String>
where
    M: rig::completion::CompletionModel + 'static,
    M::StreamingResponse: rig::completion::GetTokenUsage + rig::wasm_compat::WasmCompatSend,
{
    let mut stream = agent.stream_prompt(input).await;
    let mut stdout = io::stdout();
    let mut output = String::new();
    let mut wrote_text = false;

    while let Some(item) = stream.next().await {
        match item? {
            rig::agent::MultiTurnStreamItem::StreamAssistantItem(
                StreamedAssistantContent::Text(text),
            ) => {
                wrote_text = true;
                output.push_str(&text.text);
                print!("{}", text.text);
                stdout.flush()?;
            }
            rig::agent::MultiTurnStreamItem::FinalResponse(final_response) => {
                if !wrote_text && !final_response.response().is_empty() {
                    output.push_str(final_response.response());
                    print!("{}", final_response.response());
                    stdout.flush()?;
                }
            }
            _ => {}
        }
    }

    println!();
    Ok(output)
}
