use crate::client::utils::elapsed_ms_u64;
use crate::client::AIClient;
use crate::types::{ConnectionTestMessageCode, ConnectionTestResult, Message, ToolDefinition};
use anyhow::Result;
use log::debug;

pub(crate) fn image_test_response_matches_expected(response: &str) -> bool {
    let upper = response.to_ascii_uppercase();

    let letters_only: String = upper.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    if letters_only.contains(AIClient::TEST_IMAGE_EXPECTED_CODE) {
        return true;
    }

    let tokens: Vec<&str> = upper
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|s| !s.is_empty())
        .collect();

    if tokens.contains(&AIClient::TEST_IMAGE_EXPECTED_CODE) {
        return true;
    }

    let single_letter_stream: String = tokens
        .iter()
        .filter_map(|token| {
            if token.len() == 1 {
                let ch = token.chars().next()?;
                if matches!(ch, 'R' | 'G' | 'B' | 'Y') {
                    return Some(ch);
                }
            }
            None
        })
        .collect();
    if single_letter_stream.contains(AIClient::TEST_IMAGE_EXPECTED_CODE) {
        return true;
    }

    let color_word_stream: String = tokens
        .iter()
        .filter_map(|token| match *token {
            "RED" => Some('R'),
            "GREEN" => Some('G'),
            "BLUE" => Some('B'),
            "YELLOW" => Some('Y'),
            _ => None,
        })
        .collect();
    if color_word_stream.contains(AIClient::TEST_IMAGE_EXPECTED_CODE) {
        return true;
    }

    let color_letter_stream: String = upper
        .chars()
        .filter(|c| matches!(*c, 'R' | 'G' | 'B' | 'Y'))
        .collect();
    color_letter_stream.contains(AIClient::TEST_IMAGE_EXPECTED_CODE)
}

pub(crate) async fn test_connection(client: &AIClient) -> Result<ConnectionTestResult> {
    let start_time = std::time::Instant::now();

    let test_messages = vec![Message::user(
        "Call the get_weather tool for city=Beijing. Do not answer with plain text.".to_string(),
    )];
    let tools = Some(vec![ToolDefinition {
        name: "get_weather".to_string(),
        description: "Get the weather of a city".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "city": { "type": "string", "description": "The city to get the weather for" }
            },
            "required": ["city"],
            "additionalProperties": false
        }),
    }]);

    match client.send_message(test_messages, tools).await {
        Ok(response) => {
            let response_time_ms = elapsed_ms_u64(start_time);
            if response.tool_calls.is_some() {
                Ok(ConnectionTestResult {
                    success: true,
                    response_time_ms,
                    model_response: Some(response.text),
                    message_code: None,
                    error_details: None,
                })
            } else {
                Ok(ConnectionTestResult {
                    success: true,
                    response_time_ms,
                    model_response: Some(response.text),
                    message_code: Some(ConnectionTestMessageCode::ToolCallsNotDetected),
                    error_details: None,
                })
            }
        }
        Err(e) => {
            let response_time_ms = elapsed_ms_u64(start_time);
            let error_msg = format!("{}", e);
            debug!("test connection failed: {}", error_msg);
            Ok(ConnectionTestResult {
                success: false,
                response_time_ms,
                model_response: None,
                message_code: None,
                error_details: Some(error_msg),
            })
        }
    }
}

pub(crate) async fn test_image_input_connection(client: &AIClient) -> Result<ConnectionTestResult> {
    let start_time = std::time::Instant::now();
    let provider = client.config.format.to_ascii_lowercase();
    let prompt = "Inspect the attached image and reply with exactly one 4-letter code for quadrant colors in TL,TR,BL,BR order using letters R,G,B,Y (R=red, G=green, B=blue, Y=yellow).";

    let content = if provider == "anthropic" {
        serde_json::json!([
            {
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": "image/png",
                    "data": AIClient::TEST_IMAGE_PNG_BASE64
                }
            },
            {
                "type": "text",
                "text": prompt
            }
        ])
    } else {
        serde_json::json!([
            {
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", AIClient::TEST_IMAGE_PNG_BASE64)
                }
            },
            {
                "type": "text",
                "text": prompt
            }
        ])
    };

    let test_messages = vec![Message {
        role: "user".to_string(),
        content: Some(content.to_string()),
        reasoning_content: None,
        thinking_signature: None,
        tool_calls: None,
        tool_call_id: None,
        name: None,
        is_error: None,
        tool_image_attachments: None,
    }];

    match client.send_message(test_messages, None).await {
        Ok(response) => {
            if image_test_response_matches_expected(&response.text) {
                Ok(ConnectionTestResult {
                    success: true,
                    response_time_ms: elapsed_ms_u64(start_time),
                    model_response: Some(response.text),
                    message_code: None,
                    error_details: None,
                })
            } else {
                let detail = format!(
                    "Image understanding verification failed: expected code '{}', got response '{}'",
                    AIClient::TEST_IMAGE_EXPECTED_CODE,
                    response.text
                );
                debug!("test image input connection failed: {}", detail);
                Ok(ConnectionTestResult {
                    success: false,
                    response_time_ms: elapsed_ms_u64(start_time),
                    model_response: Some(response.text),
                    message_code: Some(ConnectionTestMessageCode::ImageInputCheckFailed),
                    error_details: Some(detail),
                })
            }
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            debug!("test image input connection failed: {}", error_msg);
            Ok(ConnectionTestResult {
                success: false,
                response_time_ms: elapsed_ms_u64(start_time),
                model_response: None,
                message_code: None,
                error_details: Some(error_msg),
            })
        }
    }
}
