use crate::types::ReasoningMode;

pub(crate) fn is_dashscope_url(url: &str) -> bool {
    url.contains("dashscope.aliyuncs.com")
}

pub(crate) fn is_siliconflow_url(url: &str) -> bool {
    url.contains("api.siliconflow.cn")
}

pub(crate) fn parse_glm_major_minor(model_name: &str) -> Option<(u32, u32)> {
    let lower = model_name.to_ascii_lowercase();
    let tail = lower.strip_prefix("glm-")?;
    let mut parts = tail.split('-');
    let version = parts.next()?;

    let mut version_parts = version.split('.');
    let major = version_parts.next()?.parse().ok()?;
    let minor = version_parts
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);

    Some((major, minor))
}

pub(crate) fn should_append_tool_stream(url: &str, model_name: &str) -> bool {
    if url.contains("bigmodel.cn") {
        return true;
    }

    if !url.contains("aliyuncs.com") {
        return false;
    }

    parse_glm_major_minor(model_name)
        .is_some_and(|(major, minor)| major > 4 || (major == 4 && minor >= 5))
}

pub(crate) fn apply_openai_compatible_reasoning_fields(
    request_body: &mut serde_json::Value,
    mode: ReasoningMode,
    url: &str,
) {
    let normalized_mode = if mode == ReasoningMode::Adaptive {
        ReasoningMode::Enabled
    } else {
        mode
    };

    if is_dashscope_url(url) || is_siliconflow_url(url) {
        if normalized_mode != ReasoningMode::Default {
            request_body["enable_thinking"] =
                serde_json::json!(normalized_mode == ReasoningMode::Enabled);
        }
        return;
    }

    match normalized_mode {
        ReasoningMode::Default => {}
        ReasoningMode::Enabled => {
            request_body["thinking"] = serde_json::json!({ "type": "enabled" });
        }
        ReasoningMode::Disabled => {
            request_body["thinking"] = serde_json::json!({ "type": "disabled" });
        }
        ReasoningMode::Adaptive => unreachable!("adaptive mode is normalized above"),
    }
}
