# Translator

A single-binary command-line translation assistant built with Rust and the
`rig` crate.

It is designed for programmers, product managers, teachers, managers, and other
knowledge workers. By default it translates between Chinese and English, detects
English word or phrase lookup locally, and streams model output.

## Install

```powershell
cargo install rig-translator-cli
```

For local development:

```powershell
cargo install --path .
```

The package name is `rig-translator-cli`; the binary is `translate`.

## Quick Start

Without config files, the tool uses DeepSeek V4 Flash and reads the API key from
the process environment:

```powershell
$env:DEEPSEEK_API_KEY = "your_api_key_here"
translate "你好，世界！"
```

Use `--direct` for one-shot output after the model finishes:

```powershell
translate "你好，世界！" --direct
```

If no text argument is provided, input is read from standard input:

```powershell
"AI will reshape the global technology industry." | translate
```

Running `translate` without input in an interactive terminal prints the built-in
help.

## Input Modes

Default mode accepts up to 200 UTF-8 bytes:

```powershell
translate "burst"
```

Use `--long` for longer natural-language input:

```powershell
translate --long "Paste a longer paragraph here..."
```

Use `--file` for UTF-8 text files:

```powershell
translate --file .\note.txt --target English
```

Obvious non-linguistic input, such as URLs, paths, hashes, JSON, config
fragments, or code, is skipped locally.

## Recent Queries

Show recent unique queries from the local cache without calling the model:

```powershell
translate --recent 10
translate --recent-words 10
translate --recent-sentences 10
```

Repeated queries are shown once, ordered by most recent use. `--recent` includes
word, phrase, and sentence queries. `--recent-words` shows only word lookups, and
`--recent-sentences` shows only sentence translations.

## Configuration

Configuration priority:

1. CLI options
2. `.env` in the current working directory
3. `~/.translator.env`
4. Process environment variables
5. Built-in defaults

Example `.env`:

```text
TRANSLATOR_PROVIDER=deepseek
TRANSLATOR_MODEL=deepseek-v4-flash
TRANSLATOR_API_KEY=your_api_key_here
TRANSLATOR_BASE_URL=https://api.deepseek.com
TARGET_LANG=auto
TRANSLATOR_CACHE=true
TRANSLATOR_CACHE_TTL_DAYS=30
TRANSLATOR_CACHE_MAX_MB=10
```

Supported providers:

```text
deepseek   DEEPSEEK_API_KEY   DEEPSEEK_MODEL   DEEPSEEK_API_BASE
openai     OPENAI_API_KEY     OPENAI_MODEL     OPENAI_BASE_URL
claude     ANTHROPIC_API_KEY  ANTHROPIC_MODEL  ANTHROPIC_API_BASE
zhipu      ZAI_API_KEY        ZAI_MODEL        ZAI_API_BASE
```

The generic keys `TRANSLATOR_PROVIDER`, `TRANSLATOR_MODEL`,
`TRANSLATOR_API_KEY`, and `TRANSLATOR_BASE_URL` work for all providers.

Useful CLI overrides:

```powershell
translate "Make this sentence more natural." --provider openai --model gpt-4o --target Chinese
translate "burst" --no-cache
```

## Cache

The cache is enabled by default and stored under the user home directory:

```text
~/.translator/cache.jsonl
```

Entries expire after 30 days by default. When the cache file exceeds 10 MB, the
least recently used entries are removed first.

Set `TRANSLATOR_CACHE_PATH` to override the cache path.

## Output

Sentence input returns only the best translation.

English word or phrase input returns a compact dictionary-style entry with
pronunciation, meanings, etymology, usage, forms, and examples.

On request failure, the tool prints a retry warning, retries once, and then
returns an error if the second attempt also fails.
