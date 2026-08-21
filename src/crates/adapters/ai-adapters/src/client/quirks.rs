pub(crate) fn is_dashscope_url(url: &str) -> bool {
    url.contains("dashscope.aliyuncs.com") || url.contains("dashscope-intl.aliyuncs.com")
}

pub(crate) fn is_siliconflow_url(url: &str) -> bool {
    url.contains("api.siliconflow.cn") || url.contains("api.siliconflow.com")
}

/// CodeBuddy's Tencent backend (`copilot.tencent.com`). Reasoning is toggled
/// through the OpenAI-compatible `enable_thinking` body field.
pub(crate) fn is_codebuddy_url(url: &str) -> bool {
    url.contains("copilot.tencent.com")
}

pub(crate) fn is_deepseek_url(url: &str) -> bool {
    url.contains("api.deepseek.com")
}

pub(crate) fn is_zhipuai_url(url: &str) -> bool {
    reqwest::Url::parse(url.trim()).ok().is_some_and(|url| {
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && url.host_str().is_some_and(|host| {
                matches!(host.trim_end_matches('.'), "open.bigmodel.cn" | "api.z.ai")
            })
    })
}

pub(crate) fn is_deepseek_reasoning_effort_model(model_name: &str) -> bool {
    matches!(
        model_name.trim().to_ascii_lowercase().as_str(),
        "deepseek-v4-flash" | "deepseek-v4-pro"
    )
}

pub(crate) fn is_glm_52_reasoning_effort_model(model_name: &str) -> bool {
    matches!(
        model_name.trim().to_ascii_lowercase().as_str(),
        "glm-5.2" | "glm-5-2" | "glm-5p2"
    )
}

pub(crate) fn normalize_deepseek_reasoning_effort(
    model_name: &str,
    effort: &str,
) -> Option<&'static str> {
    let is_flash = model_name.trim().eq_ignore_ascii_case("deepseek-v4-flash");
    match effort.trim().to_ascii_lowercase().as_str() {
        "" => None,
        "low" if is_flash => Some("low"),
        "high" => Some("high"),
        "max" => Some("max"),
        "low" | "medium" => Some("high"),
        "xhigh" => Some("max"),
        "none" | "minimal" => None,
        _ => None,
    }
}

pub(crate) fn normalize_glm_52_reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "high" | "low" | "medium" => Some("high"),
        "max" | "xhigh" => Some("max"),
        "" | "none" | "minimal" => None,
        _ => None,
    }
}

/// Normalizes a reasoning effort string to the conservative CodeBuddy tier
/// set (`high` / `max`), mirroring the `thinkingLevelMap` facts for the common
/// CodeBuddy reasoning models (glm-5.2, deepseek-v4-*). Lower generic values
/// collapse up to `high` because the Tencent gateway exposes only high/max.
pub(crate) fn normalize_codebuddy_reasoning_effort(effort: &str) -> Option<&'static str> {
    match effort.trim().to_ascii_lowercase().as_str() {
        "high" | "low" | "medium" => Some("high"),
        "max" | "xhigh" => Some("max"),
        _ => None,
    }
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
    if url.contains("bigmodel.cn") || url.contains("api.z.ai") {
        return true;
    }

    if !url.contains("aliyuncs.com") {
        return false;
    }

    parse_glm_major_minor(model_name)
        .is_some_and(|(major, minor)| major > 4 || (major == 4 && minor >= 5))
}

pub(crate) fn apply_openai_compatible_toggle(
    request_body: &mut serde_json::Value,
    enabled: bool,
    url: &str,
) -> bool {
    if is_dashscope_url(url) || is_siliconflow_url(url) || is_codebuddy_url(url) {
        request_body["enable_thinking"] = serde_json::json!(enabled);
        return true;
    }
    if is_deepseek_url(url) {
        request_body["thinking"] = serde_json::json!({
            "type": if enabled { "enabled" } else { "disabled" }
        });
        return true;
    }
    false
}
