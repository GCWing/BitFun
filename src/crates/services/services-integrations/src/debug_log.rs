//! Debug log network provider.

use serde_json::Value;

pub async fn post_debug_log(url: &str, log_line: &Value) -> Result<(), String> {
    reqwest::Client::new()
        .post(url)
        .json(log_line)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    Ok(())
}
