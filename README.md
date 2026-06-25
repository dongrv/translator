# Translator

A single-binary command-line translation assistant built with Rust and the `rig` crate.

The program reads text from standard input, validates the input locally, and asks
DeepSeek V4 Pro to produce a concise command-line style translation result.

No prompt or configuration file is required at runtime. The translation policy is
embedded in the binary.

## Run

Set your DeepSeek API key first:

```powershell
$env:DEEPSEEK_API_KEY = "your_api_key_here"
```

By default, output is streamed:

```powershell
cargo run -- "AI将重构世界科技行业格局。"
```

Use `--direct` for one-shot output after the model finishes:

```powershell
cargo run -- "你好，世界！" --direct
```

If no text argument is provided, input is read from standard input:

```powershell
"AI将重构世界科技行业格局。" | cargo run
```

Or run the binary and type/paste text, then end input with `Ctrl+Z` and Enter on Windows.

## Example

```text
AI will reshape the landscape of the global technology industry.
```
