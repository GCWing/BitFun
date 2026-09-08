//! OpenAI-compatible Router transport and classifier protocol.
//!
//! The consumer supplies isolated input and owns thresholds, model slots, context,
//! traces, and fallback policy. This adapter has no session or config IO.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

const SIMPLE_TOKEN_ID: u32 = 22_944;
const NON_SIMPLE_FIRST_TOKEN_ID: u32 = 6_280;
const ROUTER_TOP_LOGPROBS: usize = 20;

#[derive(Debug)]
pub enum RouterClientError {
    Http(String),
    Deserialization(String),
}

impl std::fmt::Display for RouterClientError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(message) | Self::Deserialization(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for RouterClientError {}

#[derive(Debug)]
pub struct RouterPrediction {
    pub generated_simple: bool,
    pub simple_probability: Option<f64>,
    pub recovered: bool,
    pub raw_output: String,
    pub usage: Option<Value>,
}

pub struct RouterHttpClient {
    client: reqwest::Client,
    endpoint: String,
    model: String,
    system_prompt: String,
    api_key: Option<String>,
}

impl RouterHttpClient {
    pub fn new(
        endpoint: String,
        model: String,
        system_prompt: String,
        api_key: Option<String>,
        timeout: Duration,
    ) -> Result<Self, RouterClientError> {
        let client = crate::client::http::create_router_http_client(timeout)
            .map_err(|error| RouterClientError::Http(error.to_string()))?;
        Ok(Self {
            client,
            endpoint,
            model,
            system_prompt,
            api_key,
        })
    }

    /// Make one bounded request. The runtime owns retries and fallback.
    pub async fn predict(&self, input: &str) -> Result<RouterPrediction, RouterClientError> {
        let body = OpenAiChatRequest {
            model: &self.model,
            messages: [
                OpenAiChatMessage {
                    role: "system",
                    content: &self.system_prompt,
                },
                OpenAiChatMessage {
                    role: "user",
                    content: input,
                },
            ],
            temperature: 0.0,
            top_p: 1.0,
            max_tokens: 128,
            stop: ["\n"],
            logprobs: true,
            top_logprobs: ROUTER_TOP_LOGPROBS,
            return_token_ids: true,
            return_tokens_as_token_ids: true,
            chat_template_kwargs: json!({"enable_thinking": false}),
        };
        let mut request = self.client.post(&self.endpoint).json(&body);
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.bearer_auth(api_key);
        }
        let response = request
            .send()
            .await
            .map_err(|error| RouterClientError::Http(format!("Router request failed: {error}")))?
            .error_for_status()
            .map_err(|error| RouterClientError::Http(format!("Router returned an error: {error}")))?
            .json::<OpenAiChatResponse>()
            .await
            .map_err(|error| {
                RouterClientError::Deserialization(format!(
                    "Failed to decode Router response: {error}"
                ))
            })?;
        let choice = response.choices.first().ok_or_else(|| {
            RouterClientError::Deserialization(
                "Router response did not contain a choice".to_string(),
            )
        })?;
        let raw_output = choice.message.content.as_deref().ok_or_else(|| {
            RouterClientError::Deserialization(
                "Router response did not contain assistant content".to_string(),
            )
        })?;
        let (generated_simple, recovered) = parse_router_route(raw_output)?;
        Ok(RouterPrediction {
            generated_simple,
            simple_probability: extract_simple_probability(choice, raw_output, generated_simple),
            recovered,
            raw_output: raw_output.to_string(),
            usage: response.usage,
        })
    }
}

#[derive(Debug, Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: [OpenAiChatMessage<'a>; 2],
    temperature: f32,
    top_p: f32,
    max_tokens: usize,
    stop: [&'a str; 1],
    logprobs: bool,
    top_logprobs: usize,
    return_token_ids: bool,
    return_tokens_as_token_ids: bool,
    chat_template_kwargs: Value,
}

#[derive(Debug, Serialize)]
struct OpenAiChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChatChoice>,
    #[serde(default)]
    usage: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatChoice {
    message: OpenAiResponseMessage,
    #[serde(default)]
    logprobs: Option<OpenAiChatLogprobs>,
    #[serde(default)]
    token_ids: Option<Vec<u32>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatLogprobs {
    content: Option<Vec<OpenAiChatLogprobContent>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatLogprobContent {
    bytes: Option<Vec<u8>>,
    top_logprobs: Vec<OpenAiTokenLogprob>,
}

#[derive(Debug, Deserialize)]
struct OpenAiTokenLogprob {
    token: String,
    logprob: f64,
}

fn extract_simple_probability(
    choice: &OpenAiChatChoice,
    raw_output: &str,
    generated_route: bool,
) -> Option<f64> {
    let token_ids = choice.token_ids.as_deref()?;
    let logprobs = choice.logprobs.as_ref()?.content.as_deref()?;
    if token_ids.len() != logprobs.len() {
        return None;
    }

    let generated_label_token = match generated_route {
        true => SIMPLE_TOKEN_ID,
        false => NON_SIMPLE_FIRST_TOKEN_ID,
    };
    let label_byte_offset = simple_type_value_byte_offset(raw_output)?;
    let label_index = token_index_at_byte_offset(logprobs, label_byte_offset)?;
    if token_ids.get(label_index).copied()? != generated_label_token {
        return None;
    }
    let candidates = &logprobs.get(label_index)?.top_logprobs;
    let simple_logprob = find_token_logprob(candidates, SIMPLE_TOKEN_ID)?;
    let non_simple_logprob = find_token_logprob(candidates, NON_SIMPLE_FIRST_TOKEN_ID)?;

    // Renormalize over the two routing classes only. Log-probabilities differ
    // from raw logits by the same log-softmax constant, so this is equivalent
    // to a two-class softmax over their logits.
    let maximum = simple_logprob.max(non_simple_logprob);
    let simple_weight = (simple_logprob - maximum).exp();
    let non_simple_weight = (non_simple_logprob - maximum).exp();
    Some(simple_weight / (simple_weight + non_simple_weight))
}

fn simple_type_value_byte_offset(raw_output: &str) -> Option<usize> {
    let first_line = raw_output.lines().next()?;
    let key_offset = first_line.find("\"simple_type\"")?;
    let after_key_offset = key_offset + "\"simple_type\"".len();
    let after_key = &first_line[after_key_offset..];
    let colon_offset = after_key.find(':')?;
    let after_colon_offset = after_key_offset + colon_offset + 1;
    let whitespace_bytes = first_line[after_colon_offset..]
        .len()
        .saturating_sub(first_line[after_colon_offset..].trim_start().len());
    let opening_quote_offset = after_colon_offset + whitespace_bytes;
    if first_line.as_bytes().get(opening_quote_offset) != Some(&b'"') {
        return None;
    }
    Some(opening_quote_offset + 1)
}

fn token_index_at_byte_offset(
    logprobs: &[OpenAiChatLogprobContent],
    target_offset: usize,
) -> Option<usize> {
    let mut current_offset: usize = 0;
    for (index, token) in logprobs.iter().enumerate() {
        let token_bytes = token.bytes.as_deref()?;
        let next_offset = current_offset.checked_add(token_bytes.len())?;
        if (current_offset..next_offset).contains(&target_offset) {
            return Some(index);
        }
        current_offset = next_offset;
    }
    None
}

fn find_token_logprob(candidates: &[OpenAiTokenLogprob], token_id: u32) -> Option<f64> {
    let expected = format!("token_id:{token_id}");
    candidates
        .iter()
        .find(|candidate| candidate.token == expected)
        .map(|candidate| candidate.logprob)
}

fn parse_router_route(raw_output: &str) -> Result<(bool, bool), RouterClientError> {
    let first_line = raw_output.lines().next().unwrap_or_default().trim();
    if first_line.is_empty() || !first_line.starts_with('{') {
        return Err(RouterClientError::Deserialization(
            "Router output is not a JSON object".to_string(),
        ));
    }
    if first_line.matches("\"simple_type\"").count() != 1 {
        return Err(RouterClientError::Deserialization(
            "Router output must contain exactly one simple_type key".to_string(),
        ));
    }
    let (prediction, recovered) = match serde_json::from_str::<Value>(first_line) {
        Ok(value) => (value, false),
        Err(_) if !first_line.ends_with('}') => {
            let repaired = format!("{first_line}}}");
            let value = serde_json::from_str::<Value>(&repaired).map_err(|error| {
                RouterClientError::Deserialization(format!(
                    "Router output is invalid JSON: {error}"
                ))
            })?;
            (value, true)
        }
        Err(error) => {
            return Err(RouterClientError::Deserialization(format!(
                "Router output is invalid JSON: {error}"
            )));
        }
    };
    let simple_type = prediction
        .as_object()
        .and_then(|object| object.get("simple_type"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            RouterClientError::Deserialization(
                "Router output is missing a string simple_type".to_string(),
            )
        })?;
    match simple_type {
        "simple" => Ok((true, recovered)),
        "non_simple" => Ok((false, recovered)),
        value => Err(RouterClientError::Deserialization(format!(
            "Router output has unsupported simple_type: {value}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn http_transport_preserves_auth_protocol_and_error_categories() {
        use axum::{
            extract::Json,
            http::{HeaderMap, StatusCode},
            routing::post,
            Router,
        };

        let app = Router::new().route("/v1/chat/completions", post(
            |headers: HeaderMap, Json(body): Json<Value>| async move {
                assert_eq!(body["messages"][0]["content"], "fixed system");
                assert_eq!(body["messages"][1]["content"], "isolated input");
                assert_eq!(body["temperature"], 0.0);
                assert_eq!(body["top_p"], 1.0);
                assert_eq!(body["max_tokens"], 128);
                assert_eq!(body["stop"], json!(["\n"]));
                assert_eq!(body["top_logprobs"], 20);
                assert_eq!(body["logprobs"], true);
                assert_eq!(body["return_token_ids"], true);
                assert_eq!(body["return_tokens_as_token_ids"], true);
                assert_eq!(body["chat_template_kwargs"], json!({"enable_thinking": false}));
                assert!(body.get("tools").is_none());
                let model = body["model"].as_str().unwrap();
                if model == "authenticated" {
                    assert_eq!(headers["authorization"], "Bearer fixture-key");
                } else {
                    assert!(!headers.contains_key("authorization"));
                }
                let valid = json!({"choices": [{"message": {"content": "{\"simple_type\":\"simple\"}"}}], "usage": {"prompt_tokens": 42}});
                match model {
                    "http_error" => (StatusCode::SERVICE_UNAVAILABLE, String::from("unavailable")),
                    "bad_json" => (StatusCode::OK, String::from("not json")),
                    "empty_choices" => (StatusCode::OK, json!({"choices": []}).to_string()),
                    "missing_content" => (StatusCode::OK, json!({"choices": [{"message": {}}]}).to_string()),
                    "invalid_output" => (StatusCode::OK, json!({"choices": [{"message": {"content": "not a prediction"}}]}).to_string()),
                    "timeout" => {
                        tokio::time::sleep(Duration::from_millis(300)).await;
                        (StatusCode::OK, valid.to_string())
                    }
                    _ => (StatusCode::OK, valid.to_string()),
                }
            }
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!(
            "http://{}/v1/chat/completions",
            listener.local_addr().unwrap()
        );
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        for model in [
            "unauthenticated",
            "authenticated",
            "http_error",
            "bad_json",
            "empty_choices",
            "missing_content",
            "invalid_output",
            "timeout",
        ] {
            let timeout = if model == "timeout" {
                Duration::from_millis(30)
            } else {
                Duration::from_secs(3)
            };
            let client = RouterHttpClient::new(
                endpoint.clone(),
                model.into(),
                "fixed system".into(),
                (model == "authenticated").then(|| "fixture-key".into()),
                timeout,
            )
            .unwrap();
            let result = client.predict("isolated input").await;
            match model {
                "unauthenticated" | "authenticated" => {
                    let prediction = result.unwrap();
                    assert!(prediction.generated_simple);
                    assert_eq!(prediction.simple_probability, None);
                    assert!(!prediction.recovered);
                    assert_eq!(prediction.usage.unwrap()["prompt_tokens"], 42);
                }
                "http_error" | "timeout" => assert!(
                    matches!(result, Err(RouterClientError::Http(_))),
                    "{model}: {result:?}"
                ),
                _ => assert!(
                    matches!(result, Err(RouterClientError::Deserialization(_))),
                    "{model}: {result:?}"
                ),
            }
        }
        server.abort();
        let _ = server.await;
    }
    #[test]
    fn parser_maps_simple_type_and_repairs_only_a_missing_closing_brace() {
        let simple = r#"{"phase":"editing","phase_detail":"production_change_applied","simple_type":"simple"}"#;
        let non_simple = r#"{"phase":"localization","phase_detail":"root_cause_closed","simple_type":"non_simple""#;

        assert_eq!(parse_router_route(simple).unwrap(), (true, false));
        assert_eq!(parse_router_route(non_simple).unwrap(), (false, true));
        assert!(parse_router_route("explanation first").is_err());
    }

    #[test]
    fn confidence_is_renormalized_over_simple_and_non_simple() {
        let raw_output = r#"{"simple_type":"simple"}"#;
        let choice = OpenAiChatChoice {
            message: OpenAiResponseMessage { content: None },
            logprobs: Some(OpenAiChatLogprobs {
                content: Some(vec![
                    OpenAiChatLogprobContent {
                        bytes: Some(br#"{"simple_type":""#.to_vec()),
                        top_logprobs: Vec::new(),
                    },
                    OpenAiChatLogprobContent {
                        bytes: Some(b"simple".to_vec()),
                        top_logprobs: vec![
                            OpenAiTokenLogprob {
                                token: format!("token_id:{SIMPLE_TOKEN_ID}"),
                                logprob: -0.2,
                            },
                            OpenAiTokenLogprob {
                                token: format!("token_id:{NON_SIMPLE_FIRST_TOKEN_ID}"),
                                logprob: -1.4,
                            },
                        ],
                    },
                    OpenAiChatLogprobContent {
                        bytes: Some(br#""}"#.to_vec()),
                        top_logprobs: Vec::new(),
                    },
                ]),
            }),
            token_ids: Some(vec![1, SIMPLE_TOKEN_ID, 2]),
        };

        let probability = extract_simple_probability(&choice, raw_output, true).unwrap();
        assert!((probability - 0.768_524_783_5).abs() < 1e-9);
    }

    #[test]
    fn missing_class_logprob_has_no_confidence() {
        let raw_output = r#"{"simple_type":"simple"}"#;
        let choice = OpenAiChatChoice {
            message: OpenAiResponseMessage { content: None },
            logprobs: Some(OpenAiChatLogprobs {
                content: Some(vec![
                    OpenAiChatLogprobContent {
                        bytes: Some(br#"{"simple_type":""#.to_vec()),
                        top_logprobs: Vec::new(),
                    },
                    OpenAiChatLogprobContent {
                        bytes: Some(b"simple".to_vec()),
                        top_logprobs: vec![OpenAiTokenLogprob {
                            token: format!("token_id:{SIMPLE_TOKEN_ID}"),
                            logprob: -0.2,
                        }],
                    },
                    OpenAiChatLogprobContent {
                        bytes: Some(br#""}"#.to_vec()),
                        top_logprobs: Vec::new(),
                    },
                ]),
            }),
            token_ids: Some(vec![1, SIMPLE_TOKEN_ID, 2]),
        };

        assert!(extract_simple_probability(&choice, raw_output, true).is_none());
    }
}
