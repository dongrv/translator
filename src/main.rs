use anyhow::Result;
use clap::Parser;

mod cli;
mod input;
mod prompt;
mod translator;

use cli::Cli;
use input::{read_stdin_if_needed, validate_input, InputDecision};
use translator::Translator;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let raw_input = read_stdin_if_needed(cli.input_text())?;

    match validate_input(&raw_input) {
        InputDecision::Translate(text) => {
            let translator = Translator::from_env()?;
            if cli.direct {
                let output = translator.translate_direct(text).await?;
                println!("{}", output.trim());
            } else {
                translator.translate_streaming(text).await?;
            }
        }
        InputDecision::Error(message) | InputDecision::Skip(message) => {
            println!("{message}");
        }
    }

    Ok(())
}
