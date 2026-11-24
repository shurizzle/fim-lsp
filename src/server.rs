use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use tower_lsp_server::jsonrpc::{Error, Result};
use tower_lsp_server::ls_types::*;
use tower_lsp_server::{Client, LanguageServer};

use crate::cache::{self, Cache};
use crate::document::Document;
use crate::provider::Provider;

const DEBOUNCE: Duration = Duration::from_millis(150);

type Pending = Shared<BoxFuture<'static, Option<Arc<str>>>>;

struct State {
    http: reqwest::Client,
    provider: OnceLock<Provider>,
    cache: Cache,
    in_flight: Mutex<HashMap<u64, Pending>>,
}

pub struct Backend {
    client: Client,
    docs: Mutex<HashMap<Uri, Document>>,
    state: Arc<State>,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            docs: Mutex::default(),
            state: Arc::new(State {
                http: reqwest::Client::new(),
                provider: OnceLock::new(),
                cache: Cache::new(),
                in_flight: Mutex::default(),
            }),
        }
    }

    pub async fn inline_completion(
        &self,
        params: InlineCompletionParams,
    ) -> Result<Option<InlineCompletionResponse>> {
        let pos = params.text_document_position;
        let uri = pos.text_document.uri;
        let Some((prefix, suffix, generation)) = ({
            let mut docs = self.docs.lock().unwrap();
            docs.get_mut(&uri).map(|doc| {
                doc.generation += 1;
                let (p, s) = doc.context(doc.char_idx(pos.position));
                (p, s, doc.generation)
            })
        }) else {
            return Ok(None);
        };

        let completion = match self.state.cache.get(&prefix, &suffix) {
            Some(c) => {
                tracing::debug!("cache hit");
                c
            }
            None => {
                // Invoked requests skip the debounce.
                if params.context.trigger_kind != InlineCompletionTriggerKind::INVOKED {
                    tokio::time::sleep(DEBOUNCE).await;
                    let stale = self
                        .docs
                        .lock()
                        .unwrap()
                        .get(&uri)
                        .is_none_or(|d| d.generation != generation);
                    if stale {
                        return Ok(None);
                    }
                }
                match self.fetch(prefix, suffix).await {
                    Some(c) => c,
                    None => return Ok(None),
                }
            }
        };

        if completion.is_empty() {
            return Ok(None);
        }
        Ok(Some(InlineCompletionResponse::Array(vec![
            InlineCompletionItem {
                insert_text: completion.to_string(),
                filter_text: None,
                range: None,
                command: None,
                insert_text_format: None,
            },
        ])))
    }

    /// Runs the request in a detached task (so a cancelled LSP request still
    /// fills the cache) and dedups identical concurrent requests.
    fn fetch(&self, prefix: String, suffix: String) -> Pending {
        let key = cache::key(&prefix, &suffix);
        let mut in_flight = self.state.in_flight.lock().unwrap();
        if let Some(p) = in_flight.get(&key) {
            return p.clone();
        }
        let state = self.state.clone();
        let task = tokio::spawn(async move {
            // Set in `initialize`, which the protocol guarantees runs first.
            let provider = state.provider.get().expect("initialized");
            let res = provider.complete(&state.http, &prefix, &suffix).await;
            state.in_flight.lock().unwrap().remove(&key);
            let text = match res {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("provider: {e}");
                    return None;
                }
            };
            let text = text.trim_end();
            // Drop completions that only repeat what already follows the cursor.
            let completion: Arc<str> = if suffix.starts_with(text) {
                "".into()
            } else {
                text.into()
            };
            state.cache.insert(key, completion.clone());
            Some(completion)
        });
        let pending = task.map(|r| r.ok().flatten()).boxed().shared();
        in_flight.insert(key, pending.clone());
        pending
    }
}

impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let options = params.initialization_options.unwrap_or_default();
        let provider: Provider = serde_json::from_value(options)
            .map_err(|e| Error::invalid_params(format!("initializationOptions: {e}")))?;
        let _ = self.state.provider.set(provider);
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                inline_completion_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "fim-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.docs
            .lock()
            .unwrap()
            .insert(doc.uri, Document::new(&doc.text));
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let mut docs = self.docs.lock().unwrap();
        if let Some(doc) = docs.get_mut(&params.text_document.uri) {
            for change in params.content_changes {
                doc.apply(change);
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.docs.lock().unwrap().remove(&params.text_document.uri);
    }
}
