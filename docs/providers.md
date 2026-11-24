# FIM provider notes

Notes from memory, not verified against current docs: check pricing, limits
and `suffix` support before relying on any of them.

## Dedicated FIM endpoints

- **Mistral Codestral**: `POST /v1/fim/completions` (`prompt` + `suffix`).
  Also available on Vertex AI and Azure.
- **DeepSeek**: `POST https://api.deepseek.com/beta/completions` with
  `suffix`. Beta, max 4K tokens.
- **Inception Mercury Coder**: dedicated FIM endpoint, diffusion model, low
  latency.
- **OpenAI**: `suffix` only on legacy `/v1/completions`
  (`gpt-3.5-turbo-instruct`). Effectively abandoned.

## Raw completions on open models

No dedicated FIM endpoint: FIM via OpenAI-compatible `/v1/completions`, either
with `suffix` (support varies per model) or by building the FIM tokens by hand
(e.g. `<|fim_prefix|>…<|fim_suffix|>…<|fim_middle|>` for Qwen2.5-Coder).

- **Fireworks**, **Together**: inference providers, run the models themselves.
  Raw prompt with hand-built FIM tokens works reliably.
- **OpenRouter**: router/proxy, forwards to upstream providers. FIM depends on
  the upstream provider and on the raw prompt reaching it untouched; `suffix`
  not guaranteed. Extra hop adds latency: poor fit for autocomplete.

## Self-hosted

- **llama.cpp server**: `/infill` (`input_prefix`, `input_suffix`,
  `input_extra` for repo context). Used by `llama.vim` / `llama.vscode`.
- **Ollama**: `/api/generate` with `suffix`, if the model template supports it.
- **vLLM / TGI**: raw completions with FIM tokens.

## Good FIM models

Codestral, Qwen2.5-Coder (1.5B–7B for latency), DeepSeek-Coder-V2-Lite,
StarCoder2.

Claude has no native FIM endpoint (chat only).
