use serde::{Deserialize, Serialize};

use super::{Error, send};

const DEFAULT_MODEL: &str = "deepseek-flash";
// FIM is only served by the beta endpoint.
const DEFAULT_BASE_URL: &str = "https://api.deepseek.com/beta";

/// Unset optional fields are not sent, so the API defaults apply.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    api_key: String,
    model: Option<String>,
    base_url: Option<String>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    stop: Option<Vec<String>>,
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    prompt: &'a str,
    suffix: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<&'a [String]>,
}

#[derive(Deserialize)]
struct Response {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    text: String,
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
        temperature: config.temperature,
        top_p: config.top_p,
        stop: config.stop.as_deref(),
    };
    let base_url = config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL);
    let resp: Response = send(
        http.post(format!("{}/completions", base_url.trim_end_matches('/')))
            .bearer_auth(&config.api_key)
            .json(&body),
    )
    .await?;
    Ok(resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.text)
        .unwrap_or_default())
}
