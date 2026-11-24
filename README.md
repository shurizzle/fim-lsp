# fim-lsp

Language server that serves fill-in-the-middle (FIM) code completions as
ghost text through `textDocument/inlineCompletion` (LSP 3.18).

- Works with any filetype: the server attaches to every buffer.
- Pluggable providers, selected by `type` (currently: DeepSeek).
- In-memory completion cache with typed-ahead reuse: typing through a
  suggestion, backspacing or undoing does not hit the API again.
- Debounced requests, deduplication of identical in-flight requests.

## Install

### Prebuilt binaries

Download the archive for your platform from the
[releases](https://github.com/shurizzle/fim-lsp/releases) page:

| Platform | Asset |
|---|---|
| macOS arm64 | `fim-lsp-aarch64-apple-darwin.tar.gz` |
| Linux x64 (static, musl) | `fim-lsp-x86_64-unknown-linux-musl.tar.gz` |
| Linux arm64 (static, musl) | `fim-lsp-aarch64-unknown-linux-musl.tar.gz` |
| Windows x64 | `fim-lsp-x86_64-pc-windows-msvc.zip` |
| Windows arm64 | `fim-lsp-aarch64-pc-windows-msvc.zip` |
| FreeBSD x64 | `fim-lsp-x86_64-unknown-freebsd.tar.gz` |
| FreeBSD arm64 | `fim-lsp-aarch64-unknown-freebsd.tar.gz` |

### From source

```sh
cargo install --git https://github.com/shurizzle/fim-lsp --tag v0.1.0 --locked
```

## Configuration

All settings are passed through LSP `initializationOptions`; the server
reads no environment variables and no config files. `initialize` fails with
`-32602 invalid params` if the options are missing or invalid, and unknown
fields are rejected.

### DeepSeek

| Field | Required | Default | Notes |
|---|---|---|---|
| `type` | yes | | `"deepseek"` |
| `apiKey` | yes | | |
| `model` | no | `"deepseek-flash"` | or `"deepseek-v4-pro"` |
| `baseUrl` | no | `"https://api.deepseek.com/beta"` | FIM is beta-only |
| `maxTokens` | no | API default | |
| `temperature` | no | API default (`1`) | `0` recommended for FIM |
| `topP` | no | API default | |
| `stop` | no | none | array of strings |

Optional fields that are not set are not sent, so the API defaults apply.
API reference: <https://api-docs.deepseek.com/api/create-completion/>.

## Editor setup

### Neovim (0.12+)

```lua
vim.lsp.config('fim-lsp', {
  cmd = { 'fim-lsp' },
  init_options = {
    type = 'deepseek',
    apiKey = 'sk-...', -- load it from a file outside your dotfiles
    temperature = 0,
  },
})
vim.lsp.enable('fim-lsp')
vim.lsp.inline_completion.enable()

vim.keymap.set('i', '<Tab>', function()
  if not vim.lsp.inline_completion.get() then
    return '<Tab>'
  end
end, { expr = true, desc = 'Accept inline completion' })
```

Any editor that supports `textDocument/inlineCompletion` works the same way.

## Logging

Logs go to stderr (shown by most editors in the LSP log), filtered with
`RUST_LOG`, e.g. `RUST_LOG=fim_lsp=debug` to see cache hits.

## Development

```sh
cargo test
cargo clippy --all-targets
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for how the server works and
[AGENTS.md](AGENTS.md) for contribution rules.
