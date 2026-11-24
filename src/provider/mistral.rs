use serde::{Deserialize, Serialize};

use super::{Error, send};

const DEFAULT_MODEL: &str = "codestral-latest";
// Codestral-only keys use "https://codestral.mistral.ai/v1".
const DEFAULT_BASE_URL: &str = "https://api.mistral.ai/v1";

/// Unset optional fields are not sent, so the API defaults apply.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    min_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    stop: Option<Vec<String>>,
    random_seed: Option<u64>,
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    prompt: &'a str,
    suffix: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    random_seed: Option<u64>,
}

#[derive(Deserialize)]
struct Response {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    // shortcut: assumes string content; FIM never returns chunk arrays today.
    content: Option<String>,
}

pub async fn complete(
    http: &reqwest::Client,
    config: &Config,
    prefix: &str,
    suffix: &str,
) -> Result<String, Error> {
    let body = Request {
        model: config.model.as_deref().unwrap_or(DEFAULT_MODEL),
        prompt: prefix,
        suffix,
        max_tokens: config.max_tokens,
        min_tokens: config.min_tokens,
        temperature: config.temperature,
        top_p: config.top_p,
        stop: config.stop.as_deref(),
        random_seed: config.random_seed,
    };
    let base_url = config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL);
    let resp: Response = send(
        http.post(format!(
            "{}/fim/completions",
            base_url.trim_end_matches('/')
        ))
        .bearer_auth(&config.api_key)
        .json(&body),
    )
    .await?;
    Ok(resp
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    fn client() -> reqwest::Client {
        // main installs it in the binary; tests need it too.
        let _ = rustls::crypto::ring::default_provider().install_default();
        reqwest::Client::new()
    }

    /// Serves one canned response and returns the request line, headers and body.
    fn mock(
        status: &'static str,
        response: &'static str,
    ) -> (String, std::thread::JoinHandle<(String, Value)>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut head = String::new();
            let mut len = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(v) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    len = v.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
                head.push_str(&line);
            }
            let mut body = vec![0; len];
            reader.read_exact(&mut body).unwrap();
            write!(
                stream,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{response}",
                response.len()
            )
            .unwrap();
            (head, serde_json::from_slice(&body).unwrap())
        });
        (url, handle)
    }

    #[tokio::test]
    async fn complete_roundtrip() {
        let (url, server) = mock("200 OK", r#"{"choices":[{"message":{"content":"x + 1"}}]}"#);
        let config: Config = serde_json::from_value(json!({
            "apiKey": "secret",
            "baseUrl": url,
            "maxTokens": 32,
        }))
        .unwrap();
        let out = complete(&client(), &config, "fn f(x) {", "}")
            .await
            .unwrap();
        assert_eq!(out, "x + 1");

        let (head, body) = server.join().unwrap();
        assert!(head.starts_with("POST /v1/fim/completions "), "{head}");
        assert!(
            head.to_ascii_lowercase()
                .contains("authorization: bearer secret"),
            "{head}"
        );
        assert_eq!(
            body,
            json!({
                "model": "codestral-latest",
                "prompt": "fn f(x) {",
                "suffix": "}",
                "max_tokens": 32,
            })
        );
    }

    #[tokio::test]
    async fn complete_null_content() {
        let (url, _server) = mock("200 OK", r#"{"choices":[{"message":{"content":null}}]}"#);
        let config: Config =
            serde_json::from_value(json!({"apiKey": "k", "baseUrl": url})).unwrap();
        let out = complete(&client(), &config, "a", "b").await.unwrap();
        assert_eq!(out, "");
    }

    #[tokio::test]
    async fn complete_api_error() {
        let (url, _server) = mock(
            "400 Bad Request",
            r#"{"object":"error","message":"min_tokens support is not enabled for this model"}"#,
        );
        let config: Config =
            serde_json::from_value(json!({"apiKey": "k", "baseUrl": url})).unwrap();
        let err = complete(&client(), &config, "a", "b").await.unwrap_err();
        assert_eq!(
            err.to_string(),
            "HTTP 400 Bad Request: min_tokens support is not enabled for this model"
        );
    }
}
