use clap::{CommandFactory, Parser};
use std::path::PathBuf;

use crate::{cache::RecentFilter, input::InputMode};

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

    /// Show the most recent N unique queries.
    #[arg(long, value_name = "N")]
    pub recent: Option<usize>,

    /// Show the most recent N unique word lookups.
    #[arg(long, value_name = "N")]
    pub recent_words: Option<usize>,

    /// Show the most recent N unique sentence translations.
    #[arg(long, value_name = "N")]
    pub recent_sentences: Option<usize>,

    /// Text to translate. If omitted, input is read from stdin.
    #[arg(value_name = "TEXT")]
    pub text: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentRequest {
    pub limit: usize,
    pub filter: RecentFilter,
}

impl Cli {
    pub fn print_help() -> std::io::Result<()> {
        Self::command().print_help()?;
        println!();
        Ok(())
    }

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

    pub fn recent_request(&self) -> Result<Option<RecentRequest>, &'static str> {
        let requests = [
            self.recent.map(|limit| (limit, RecentFilter::All)),
            self.recent_words.map(|limit| (limit, RecentFilter::Words)),
            self.recent_sentences
                .map(|limit| (limit, RecentFilter::Sentences)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

        if requests.is_empty() {
            return Ok(None);
        }

        if requests.len() > 1 || !self.text.is_empty() || self.file.is_some() {
            return Err("ERROR: recent query options cannot be combined with input text, --file, or each other.");
        }

        let (limit, filter) = requests[0];
        if limit == 0 {
            return Err("ERROR: recent query count must be greater than 0.");
        }

        Ok(Some(RecentRequest { limit, filter }))
    }

    pub fn should_show_help_without_input(&self, stdin_is_terminal: bool) -> bool {
        stdin_is_terminal
            && self.text.is_empty()
            && self.file.is_none()
            && self.recent.is_none()
            && self.recent_words.is_none()
            && self.recent_sentences.is_none()
    }

    pub fn should_show_help_for_empty_input(&self, input: &str) -> bool {
        self.text.is_empty()
            && self.file.is_none()
            && self.recent.is_none()
            && self.recent_words.is_none()
            && self.recent_sentences.is_none()
            && input.trim().is_empty()
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

    #[test]
    fn parses_recent_query_options() {
        let cli = Cli::parse_from(["translate", "--recent", "5"]);

        assert_eq!(
            cli.recent_request().unwrap(),
            Some(RecentRequest {
                limit: 5,
                filter: RecentFilter::All
            })
        );
    }

    #[test]
    fn rejects_conflicting_recent_query_options() {
        let cli = Cli::parse_from(["translate", "hello", "--recent", "5"]);

        assert!(cli.recent_request().is_err());
    }

    #[test]
    fn shows_help_when_no_input_and_stdin_is_terminal() {
        let cli = Cli::parse_from(["translate"]);

        assert!(cli.should_show_help_without_input(true));
        assert!(!cli.should_show_help_without_input(false));
    }

    #[test]
    fn shows_help_when_stdin_input_is_empty() {
        let cli = Cli::parse_from(["translate"]);

        assert!(cli.should_show_help_for_empty_input(""));
        assert!(!cli.should_show_help_for_empty_input("hello"));
    }
}
