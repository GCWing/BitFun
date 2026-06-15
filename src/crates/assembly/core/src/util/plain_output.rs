//! Helpers for sanitizing plain-text model outputs in contexts that must not
//! include reasoning markup.

const THINK_OPEN_TAG: &str = "<think>";
const THINK_CLOSE_TAG: &str = "</think>";

/// Remove reasoning markup from model output intended to be consumed as plain text.
///
/// Rules:
/// - Remove every complete `<think>...</think>` block.
/// - If a `<think>` block is opened but never closed, discard everything from the
///   opening tag to the end of the string.
/// - If a dangling `</think>` remains, keep only the content after the last one.
/// - Trim surrounding whitespace in the final result.
pub fn sanitize_plain_model_output(raw: &str) -> String {
    let mut cleaned = raw.to_string();

    loop {
        let Some(open_idx) = cleaned.find(THINK_OPEN_TAG) else {
            break;
        };
        let content_start = open_idx + THINK_OPEN_TAG.len();

        if let Some(relative_close_idx) = cleaned[content_start..].find(THINK_CLOSE_TAG) {
            let close_end = content_start + relative_close_idx + THINK_CLOSE_TAG.len();
            cleaned.replace_range(open_idx..close_end, "");
        } else {
            cleaned.truncate(open_idx);
            break;
        }
    }

    if let Some((_, suffix)) = cleaned.rsplit_once(THINK_CLOSE_TAG) {
        cleaned = suffix.to_string();
    }

    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::sanitize_plain_model_output;

    #[test]
    fn strips_complete_leading_think_block() {
        let result = sanitize_plain_model_output("<think>internal</think>real content");
        assert_eq!(result, "real content");
    }

    #[test]
    fn strips_multiple_think_blocks() {
        let result =
            sanitize_plain_model_output("<think>first</think>real<think>second</think> content");
        assert_eq!(result, "real content");
    }

    #[test]
    fn strips_prefix_before_dangling_closing_think_tag() {
        let result = sanitize_plain_model_output("internal chain</think>real content");
        assert_eq!(result, "real content");
    }

    #[test]
    fn drops_unclosed_think_block_tail() {
        let result = sanitize_plain_model_output("real content <think>internal");
        assert_eq!(result, "real content");
    }

    #[test]
    fn returns_empty_when_only_think_content_exists() {
        let result = sanitize_plain_model_output("<think>internal only");
        assert_eq!(result, "");
    }
}
