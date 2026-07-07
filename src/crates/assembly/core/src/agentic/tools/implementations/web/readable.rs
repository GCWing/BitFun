use crate::util::errors::{BitFunError, BitFunResult};

pub(crate) use tool_runtime::web_readable::{
    html_to_text, is_html, looks_noisy, ReadableWebOutput as ReadableOutput,
    RequestedWebFetchFormat as RequestedFormat,
};

pub(crate) fn normalize_requested_format(format: Option<&str>) -> BitFunResult<RequestedFormat> {
    tool_runtime::web_readable::normalize_requested_format(format).map_err(BitFunError::tool)
}

pub(crate) fn extract_markdown_with_text_fallback(
    html: &str,
    base_url: &str,
) -> BitFunResult<ReadableOutput> {
    tool_runtime::web_readable::extract_markdown_with_text_fallback(html, base_url)
        .map_err(BitFunError::tool)
}

pub(crate) fn extract_html_title(html: &str) -> Option<String> {
    tool_runtime::web_readable::extract_html_title(html)
}
