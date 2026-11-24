mod cache;
mod document;
mod provider;
mod server;

use tower_lsp_server::{LspService, Server};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // ring instead of aws-lc-rs: plain `cc` build, cross-compiles to every release target.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("no other rustls provider installed");

    let (service, socket) = LspService::build(server::Backend::new)
        .custom_method(
            "textDocument/inlineCompletion",
            server::Backend::inline_completion,
        )
        .finish();
    Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
        .serve(service)
        .await;
}
