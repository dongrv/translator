use anyhow::Result;
use futures::StreamExt;
use rig::client::{CompletionClient, ProviderClient};
use rig::completion::Prompt;
use rig::providers::deepseek;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use std::io::{self, Write};

use crate::prompt::SYSTEM_PROMPT;

pub struct Translator {
    agent: rig::agent::Agent<deepseek::CompletionModel>,
}

impl Translator {
    pub fn from_env() -> Result<Self> {
        let client = deepseek::Client::from_env()?;
        let agent = client
            .agent(deepseek::DEEPSEEK_V4_PRO)
            .preamble(SYSTEM_PROMPT)
            .temperature(0.2)
            .build();

        Ok(Self { agent })
    }

    pub async fn translate_direct(&self, input: &str) -> Result<String> {
        let output = self.agent.prompt(input).await?;
        Ok(output)
    }

    pub async fn translate_streaming(&self, input: &str) -> Result<()> {
        let mut stream = self.agent.stream_prompt(input).await;
        let mut stdout = io::stdout();
        let mut wrote_text = false;

        while let Some(item) = stream.next().await {
            match item? {
                rig::agent::MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::Text(text),
                ) => {
                    wrote_text = true;
                    print!("{}", text.text);
                    stdout.flush()?;
                }
                rig::agent::MultiTurnStreamItem::FinalResponse(final_response) => {
                    if !wrote_text && !final_response.response().is_empty() {
                        print!("{}", final_response.response());
                        stdout.flush()?;
                    }
                }
                _ => {}
            }
        }

        println!();
        Ok(())
    }
}
