use clap::Parser;
use std::path::PathBuf;

use crate::input::InputMode;

#[derive(Debug, Parser)]
#[command(
    name = "translate",
    version,
    about = "A compact AI-powered translation CLI"
)]
pub struct Cli {
    /// Print the response after the model finishes instead of streaming tokens.
    #[arg(long)]
    pub direct: bool,

    /// Allow longer natural-language input from args or stdin.
    #[arg(long)]
    pub long: bool,

    /// Read input from a UTF-8 text file.
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,

    /// Force the target language, for example English, Chinese, Japanese.
    #[arg(long, value_name = "LANG")]
    pub target: Option<String>,

    /// Override the model provider: deepseek, openai, claude, or zhipu.
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Override the model name.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Override the model API base URL.
    #[arg(long, value_name = "URL")]
    pub base_url: Option<String>,

    /// Disable local response cache.
    #[arg(long)]
    pub no_cache: bool,

    /// Text to translate. If omitted, input is read from stdin.
    #[arg(value_name = "TEXT")]
    pub text: Vec<String>,
}

impl Cli {
    pub fn input_text(&self) -> Option<String> {
        if self.text.is_empty() {
            None
        } else {
            Some(self.text.join(" "))
        }
    }

    pub fn input_mode(&self) -> InputMode {
        if self.file.is_some() {
            InputMode::File
        } else if self.long {
            InputMode::Long
        } else {
            InputMode::Short
        }
    }

    pub fn cache_override(&self) -> Option<bool> {
        self.no_cache.then_some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_direct_mode() {
        let cli = Cli::parse_from(["translate", "hello", "--direct"]);

        assert!(cli.direct);
        assert_eq!(cli.input_text().as_deref(), Some("hello"));
    }

    #[test]
    fn joins_multiple_text_parts() {
        let cli = Cli::parse_from(["translate", "hello", "world"]);

        assert!(!cli.direct);
        assert_eq!(cli.input_text().as_deref(), Some("hello world"));
    }

    #[test]
    fn parses_file_and_target_options() {
        let cli = Cli::parse_from([
            "translate",
            "--file",
            "note.txt",
            "--target",
            "Japanese",
            "--no-cache",
        ]);

        assert_eq!(cli.input_mode(), InputMode::File);
        assert_eq!(cli.target.as_deref(), Some("Japanese"));
        assert!(cli.no_cache);
    }
}
