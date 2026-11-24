mod deepseek;

use serde::Deserialize;

/// FIM backend, selected by the `type` field of LSP `initializationOptions`.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Provider {
    DeepSeek(deepseek::Config),
}

impl Provider {
    pub async fn complete(
        &self,
        http: &reqwest::Client,
        prefix: &str,
        suffix: &str,
    ) -> reqwest::Result<String> {
        match self {
            Self::DeepSeek(config) => deepseek::complete(http, config, prefix, suffix).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse() {
        let p: Provider =
            serde_json::from_value(json!({"type": "deepseek", "apiKey": "k"})).unwrap();
        assert!(matches!(p, Provider::DeepSeek(_)));
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
