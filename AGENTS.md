# AGENTS.md

Rules for coding agents working on this repository. Read
[ARCHITECTURE.md](ARCHITECTURE.md) first.

## Commands

```sh
cargo build
cargo test
cargo clippy --all-targets   # must be warning-free
cargo fmt
```

## Rules

- English only in code, comments, docs and commit messages.
- Configuration comes only from LSP `initializationOptions`. Do not read
  environment variables or config files (except `RUST_LOG` for logging).
- Every provider option that can be optional must be `Option` and omitted
  from the request when unset, so the API default applies.
- Keep config structs `deny_unknown_fields` so typos fail at `initialize`.
- Do not add a dependency for a few lines of code. Do not reintroduce
  `aws-lc-rs`: TLS must stay on rustls + `ring` so all release targets
  cross-compile.
- Never log prompts, completions or API keys above `debug`; never log API
  keys at all.
- New non-trivial logic gets a unit test next to it (`#[cfg(test)]`).
- Never commit, tag or push without explicit approval from the maintainer.
- Commits follow Conventional Commits.

## Releasing

1. Bump `version` in `Cargo.toml`, run `cargo build` to update `Cargo.lock`.
2. Commit, then tag `vX.Y.Z` and push the tag: the release workflow builds and
   uploads all binaries.
3. Update the version in the Mason registry spec of any consumer that pins it.
