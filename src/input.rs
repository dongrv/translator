use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{self, Read},
    path::Path,
};

pub const MAX_SHORT_INPUT_BYTES: usize = 200;
pub const MAX_LONG_INPUT_BYTES: usize = 4_000;
pub const MAX_FILE_INPUT_BYTES: usize = 64 * 1024;
pub const INPUT_TOO_LONG: &str = "ERROR: input exceeds 200 bytes.";
pub const LONG_INPUT_TOO_LONG: &str = "ERROR: input exceeds long mode limit.";
pub const FILE_INPUT_TOO_LONG: &str = "ERROR: file input exceeds limit.";
pub const NON_LINGUISTIC_INPUT: &str = "SKIP: non-linguistic input.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Short,
    Long,
    File,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputDecision<'a> {
    Translate(&'a str),
    Error(&'static str),
    Skip(&'static str),
}

pub fn read_input(cli_text: Option<String>, file: Option<&Path>) -> Result<String> {
    match (cli_text, file) {
        (Some(_), Some(_)) => bail!("ERROR: text argument cannot be used with --file."),
        (Some(text), None) => Ok(text),
        (None, Some(path)) => {
            fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
        }
        (None, None) => read_stdin().context("failed to read stdin"),
    }
}

pub fn read_stdin() -> io::Result<String> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    Ok(input)
}

pub fn validate_input(input: &str, mode: InputMode) -> InputDecision<'_> {
    let text = input.trim();

    if text.as_bytes().len() > max_input_bytes(mode) {
        return InputDecision::Error(too_long_message(mode));
    }

    if text.is_empty() || is_non_linguistic(text) {
        return InputDecision::Skip(NON_LINGUISTIC_INPUT);
    }

    InputDecision::Translate(text)
}

fn max_input_bytes(mode: InputMode) -> usize {
    match mode {
        InputMode::Short => MAX_SHORT_INPUT_BYTES,
        InputMode::Long => MAX_LONG_INPUT_BYTES,
        InputMode::File => MAX_FILE_INPUT_BYTES,
    }
}

fn too_long_message(mode: InputMode) -> &'static str {
    match mode {
        InputMode::Short => INPUT_TOO_LONG,
        InputMode::Long => LONG_INPUT_TOO_LONG,
        InputMode::File => FILE_INPUT_TOO_LONG,
    }
}

fn is_non_linguistic(input: &str) -> bool {
    let text = input.trim();

    looks_like_url(text)
        || looks_like_path(text)
        || looks_like_hash(text)
        || looks_like_json_or_config(text)
        || looks_like_code(text)
        || symbol_ratio_is_too_high(text)
}

fn looks_like_url(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("www.")
        || lower.starts_with("mailto:")
}

fn looks_like_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    let has_windows_drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/');
    let has_separator = text.contains('\\') || text.contains('/');

    has_windows_drive
        || (text.starts_with('/') && has_separator && !text.contains(' '))
        || (text.starts_with("./") && !text.contains(' '))
        || (text.starts_with("../") && !text.contains(' '))
}

fn looks_like_hash(text: &str) -> bool {
    let compact = text.trim();
    compact.len() >= 16 && compact.chars().all(|c| c.is_ascii_hexdigit())
}

fn looks_like_json_or_config(text: &str) -> bool {
    let trimmed = text.trim();
    let json_like = (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'));
    let assignment_like = trimmed.lines().count() > 1
        && trimmed
            .lines()
            .filter(|line| {
                let line = line.trim();
                !line.is_empty()
                    && (line.contains('=') || line.contains(':'))
                    && line.split_whitespace().count() <= 3
            })
            .count()
            >= 2;

    json_like || assignment_like
}

fn looks_like_code(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let code_markers = [
        "fn ",
        "let ",
        "const ",
        "var ",
        "class ",
        "import ",
        "package ",
        "func ",
        "#include",
        "select ",
        "insert ",
        "update ",
        "delete ",
        "create table",
        "<html",
        "</",
    ];

    let has_code_marker = code_markers.iter().any(|marker| lower.contains(marker));
    let has_code_punctuation = text.contains('{')
        || text.contains('}')
        || text.contains(';')
        || text.contains("=>")
        || text.contains("::")
        || text.contains("==");

    has_code_marker && has_code_punctuation
}

fn symbol_ratio_is_too_high(text: &str) -> bool {
    let total = text.chars().count();
    if total < 8 {
        return false;
    }

    let symbols = text
        .chars()
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace() && !is_cjk(*c))
        .count();

    symbols * 100 / total >= 45
}

fn is_cjk(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_natural_language() {
        assert_eq!(
            validate_input(
                "AI will reshape the global technology industry.",
                InputMode::Short
            ),
            InputDecision::Translate("AI will reshape the global technology industry.")
        );
        assert_eq!(
            validate_input("Make this sentence more natural.", InputMode::Short),
            InputDecision::Translate("Make this sentence more natural.")
        );
    }

    #[test]
    fn rejects_oversized_short_input() {
        let input = "a".repeat(MAX_SHORT_INPUT_BYTES + 1);
        assert_eq!(
            validate_input(&input, InputMode::Short),
            InputDecision::Error(INPUT_TOO_LONG)
        );
    }

    #[test]
    fn allows_long_mode_beyond_short_limit() {
        let input = "hello ".repeat(40);
        assert_eq!(
            validate_input(&input, InputMode::Long),
            InputDecision::Translate(input.trim())
        );
    }

    #[test]
    fn skips_empty_and_non_linguistic_input() {
        assert_eq!(
            validate_input("   ", InputMode::Short),
            InputDecision::Skip(NON_LINGUISTIC_INPUT)
        );
        assert_eq!(
            validate_input("https://example.com/docs", InputMode::Short),
            InputDecision::Skip(NON_LINGUISTIC_INPUT)
        );
        assert_eq!(
            validate_input("D:\\work\\app\\main.rs", InputMode::Short),
            InputDecision::Skip(NON_LINGUISTIC_INPUT)
        );
        assert_eq!(
            validate_input("a3f4c9d8e7b61234", InputMode::Short),
            InputDecision::Skip(NON_LINGUISTIC_INPUT)
        );
    }

    #[test]
    fn skips_code_and_config_like_input() {
        assert_eq!(
            validate_input("fn main() { println!(\"hi\"); }", InputMode::Short),
            InputDecision::Skip(NON_LINGUISTIC_INPUT)
        );
        assert_eq!(
            validate_input("host: localhost\nport: 8080", InputMode::Short),
            InputDecision::Skip(NON_LINGUISTIC_INPUT)
        );
    }
}
