# Architecture

## Overview

```
editor ──stdio/JSON-RPC──▶ server.rs ──▶ cache.rs ──hit──▶ response
                              │             │miss
                              ▼             ▼
                         document.rs    provider/ ──HTTP──▶ FIM API
```

Single binary, async (tokio), built on `tower-lsp-server`.

## Modules

| Path | Role |
|---|---|
| `src/main.rs` | stdio bootstrap, tracing to stderr, rustls `ring` provider, registers `textDocument/inlineCompletion` as a custom method (not in the `LanguageServer` trait). |
| `src/server.rs` | `Backend`: `initialize` parses `initializationOptions` into a `Provider`, document sync, the inline completion handler, debounce, in-flight dedup. |
| `src/document.rs` | `Document` (ropey `Rope`): incremental edits, LSP UTF-16 position to char index, prefix/suffix windows. |
| `src/cache.rs` | LRU completion cache with typed-ahead lookup. |
| `src/provider/mod.rs` | `Provider` enum, tagged by `type`, dispatches `complete`. |
| `src/provider/deepseek.rs` | DeepSeek FIM client (`POST {baseUrl}/completions`). |
| `src/provider/mistral.rs` | Mistral FIM client (`POST {baseUrl}/fim/completions`). |

## Request flow (`textDocument/inlineCompletion`)

1. Bump the document `generation` and take prefix/suffix windows around the
   cursor: up to 8192 chars before and 2048 after, cut on line boundaries so
   the prefix start does not move while the user types on the same line.
2. Cache lookup (below). A hit returns immediately.
3. On a miss, unless the trigger is `INVOKED`, sleep 150 ms; if the
   `generation` changed meanwhile, a newer request superseded this one and it
   returns `null`.
4. `fetch` spawns the provider call in a detached tokio task, shared by key:
   concurrent identical requests await the same task, and a cancelled LSP
   request still lets the result land in the cache.
5. Post-process: trim trailing whitespace; a completion that only repeats the
   text after the cursor becomes empty. The result (empty included, to avoid
   re-asking) is cached.

## Cache

- Key: hash of (prefix window, suffix window). Value: `Arc<str>`.
- Capacity: 256 entries, LRU, behind a `Mutex`.
- Lookup:
  1. Exact key: covers backspace/undo back to a previous state.
  2. Typed-ahead: for each of the last 64 typed chars, try the key of the
     prefix without them; if that completion starts with the typed text, return
     the remainder. Covers typing through a suggestion.
- Design borrowed from llama.vim / llama.vscode (ggml-org).

Known limits:
- Edits outside the windows do not invalidate entries.
- The key ignores the provider/model; fine while the provider is fixed per
  process.

## Adding a provider

1. Add `src/provider/<name>.rs` with a `Config` (`#[serde(rename_all =
   "camelCase", deny_unknown_fields)]`, optional fields as `Option`) and an
   `async fn complete(http, config, prefix, suffix) -> Result<String, Error>`
   that posts through `provider::send` (keeps the API error message).
2. Add a variant to `Provider` and a match arm in `Provider::complete`.
3. Document its options in the README.

## Release

`.github/workflows/release.yml` runs on `v*` tags and uploads one archive per
target to the GitHub release. Linux targets are static musl builds via
`cross`; `aarch64-unknown-freebsd` (tier 3) is built with `cross` on a pinned
nightly with `-Zbuild-std`. TLS uses rustls with `ring` instead of
`aws-lc-rs` so every target cross-compiles with a plain C toolchain.
