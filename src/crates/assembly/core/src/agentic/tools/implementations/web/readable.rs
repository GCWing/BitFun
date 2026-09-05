use crate::util::errors::{OpenBitFunError, OpenBitFunResult};

#[cfg(test)]
pub(crate) use tool_runtime::web_readable::{html_to_text, looks_noisy};
pub(crate) use tool_runtime::web_readable::{
    is_html, ReadableWebOutput as ReadableOutput, RequestedWebFetchFormat as RequestedFormat,
};

pub(crate) fn normalize_requested_format(
    format: Option<&str>,
) -> OpenBitFunResult<RequestedFormat> {
    tool_runtime::web_readable::normalize_requested_format(format).map_err(OpenBitFunError::tool)
}

pub(crate) fn extract_markdown_with_text_fallback(
    html: &str,
    base_url: &str,
) -> OpenBitFunResult<ReadableOutput> {
    tool_runtime::web_readable::extract_markdown_with_text_fallback(html, base_url)
        .map_err(OpenBitFunError::tool)
}

pub(crate) fn extract_html_title(html: &str) -> Option<String> {
    tool_runtime::web_readable::extract_html_title(html)
}
