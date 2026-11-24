mod deepseek;
mod mistral;

use std::fmt;

use serde::Deserialize;
use serde::de::DeserializeOwned;

/// FIM backend, selected by the `type` field of LSP `initializationOptions`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Provider {
    DeepSeek(deepseek::Config),
    Mistral(mistral::Config),
}

impl Provider {
    pub async fn complete(
        &self,
        http: &reqwest::Client,
        prefix: &str,
        suffix: &str,
    ) -> Result<String, Error> {
        match self {
            Self::DeepSeek(config) => deepseek::complete(http, config, prefix, suffix).await,
            Self::Mistral(config) => mistral::complete(http, config, prefix, suffix).await,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Http(reqwest::Error),
    /// Non-2xx response; `message` is the API's own error message, if any.
    Api {
        status: reqwest::StatusCode,
        message: Option<String>,
    },
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Self::Http(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(e) => e.fmt(f),
            Self::Api {
                status,
                message: Some(m),
            } => write!(f, "HTTP {status}: {m}"),
            Self::Api {
                status,
                message: None,
            } => write!(f, "HTTP {status}"),
        }
    }
}

/// Sends the request and decodes a JSON response, keeping the API error
/// message on non-2xx responses.
async fn send<T: DeserializeOwned>(req: reqwest::RequestBuilder) -> Result<T, Error> {
    let resp = req.send().await?;
    let status = resp.status();
    if status.is_success() {
        return Ok(resp.json().await?);
    }
    let body = resp.text().await.unwrap_or_default();
    // The full body can echo the request (e.g. validation errors), so only at debug.
    tracing::debug!("error body: {body}");
    Err(Error::Api {
        status,
        message: api_message(&body),
    })
}

/// Extracts `error.message` (OpenAI style, DeepSeek), `message` or `detail`
/// (Mistral, e.g. on 401).
fn api_message(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.pointer("/error/message")
        .or_else(|| v.get("message"))
        .or_else(|| v.get("detail"))?
        .as_str()
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn api_message_formats() {
        assert_eq!(
            api_message(r#"{"error":{"message":"bad key","type":"auth"}}"#).as_deref(),
            Some("bad key")
        );
        assert_eq!(
            api_message(r#"{"object":"error","message":"top_p must be 1"}"#).as_deref(),
            Some("top_p must be 1")
        );
        assert_eq!(
            api_message(r#"{"detail":"Invalid API Key"}"#).as_deref(),
            Some("Invalid API Key")
        );
        assert_eq!(api_message(r#"{"detail":[{"loc":"x"}]}"#), None);
        assert_eq!(api_message("<html>502</html>"), None);
    }

    #[test]
    fn parse() {
        let p: Provider =
            serde_json::from_value(json!({"type": "deepseek", "apiKey": "k"})).unwrap();
        assert!(matches!(p, Provider::DeepSeek(_)));
        let p: Provider =
            serde_json::from_value(json!({"type": "mistral", "apiKey": "k", "randomSeed": 1}))
                .unwrap();
        assert!(matches!(p, Provider::Mistral(_)));
        assert!(
            serde_json::from_value::<Provider>(json!({"type": "nope", "apiKey": "k"})).is_err()
        );
        assert!(
            serde_json::from_value::<Provider>(
                json!({"type": "deepseek", "apiKey": "k", "typo": 1})
            )
            .is_err()
        );
        assert!(serde_json::from_value::<Provider>(json!({"type": "deepseek"})).is_err());
    }
}
