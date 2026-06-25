use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "translate",
    version,
    about = "A compact DeepSeek-powered translation CLI"
)]
pub struct Cli {
    /// Print the response after the model finishes instead of streaming tokens.
    #[arg(long)]
    pub direct: bool,

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_direct_mode() {
        let cli = Cli::parse_from(["translate", "你好，世界！", "--direct"]);

        assert!(cli.direct);
        assert_eq!(cli.input_text().as_deref(), Some("你好，世界！"));
    }

    #[test]
    fn joins_multiple_text_parts() {
        let cli = Cli::parse_from(["translate", "hello", "world"]);

        assert!(!cli.direct);
        assert_eq!(cli.input_text().as_deref(), Some("hello world"));
    }
}
