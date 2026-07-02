use crate::util::errors::{BitFunError, BitFunResult};
use crate::util::truncate_at_char_boundary;
use htmd::HtmlToMarkdown;
use legible::{parse as parse_legible, Error as LegibleError, Options as LegibleOptions};
use readability_js::{Readability, ReadabilityOptions};
use regex::{Captures, Regex};

const MIN_MARKDOWN_CHARS: usize = 40;
const MIN_PLAIN_TEXT_CHARS: usize = 40;
const NOISE_MARKERS: &[&str] = &[
    "__next_f.push",
    "siteSettings",
    "\"_type\":\"reference\"",
    "<!DOCTYPE html",
    "<html",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequestedFormat {
    Raw,
    Markdown,
    Json,
}

#[derive(Debug, Clone)]
pub(crate) struct ReadableOutput {
    pub title: Option<String>,
    pub content: String,
    pub content_representation: &'static str,
    pub extractor: &'static str,
}

#[derive(Debug)]
struct ExtractedCandidate {
    title: Option<String>,
    extractor: &'static str,
    markdown: Option<String>,
    text: String,
}

type ExtractorFn = fn(&str, &str) -> BitFunResult<ExtractedCandidate>;

pub(crate) fn normalize_requested_format(format: Option<&str>) -> BitFunResult<RequestedFormat> {
    match format.unwrap_or("markdown") {
        "raw" => Ok(RequestedFormat::Raw),
        "markdown" | "text" => Ok(RequestedFormat::Markdown),
        "json" => Ok(RequestedFormat::Json),
        other => Err(BitFunError::tool(format!(
            "Unsupported format '{}'. Expected raw, markdown, or json.",
            other
        ))),
    }
}

pub(crate) fn is_html(content_type: Option<&str>, content: &str) -> bool {
    if let Some(ct) = content_type {
        let ct = ct.to_lowercase();
        if ct.contains("text/html") || ct.contains("application/xhtml") {
            return true;
        }
    }
    let sample = truncate_at_char_boundary(content, 2048);
    let sample_lower = sample.to_lowercase();
    sample_lower.contains("<!doctype html")
        || sample_lower.contains("<html")
        || sample_lower.contains("</html>")
}

pub(crate) fn extract_markdown_with_text_fallback(
    html: &str,
    base_url: &str,
) -> BitFunResult<ReadableOutput> {
    let mut plain_text_fallback: Option<ReadableOutput> = None;

    // Local extraction experiments across article, documentation, wiki, and
    // forum pages showed this order worked best for the current codebase:
    // - legible: best article quality with good latency
    // - readability-js: similar quality but slower, useful as a fallback
    // We intentionally avoid rs-trafilatura here for now. In the current crate
    // version it emits unconditional DEBUG lines to stderr during extraction,
    // which would pollute BitFun tool execution output. Revisit it once the
    // upstream crate makes that logging opt-in or routes it through a proper
    // logging facade.
    for extractor in [
        attempt_legible as ExtractorFn,
        attempt_readability_js as ExtractorFn,
    ] {
        let Ok(candidate) = extractor(html, base_url) else {
            continue;
        };

        if let Some(markdown) = candidate.markdown {
            if markdown_looks_usable(&markdown) {
                return Ok(ReadableOutput {
                    title: candidate.title,
                    content: markdown,
                    content_representation: "markdown",
                    extractor: candidate.extractor,
                });
            }
        }

        if plain_text_fallback.is_none() && plain_text_looks_usable(&candidate.text) {
            plain_text_fallback = Some(ReadableOutput {
                title: candidate.title,
                content: normalize_text(&candidate.text),
                content_representation: "plain_text",
                extractor: candidate.extractor,
            });
        }
    }

    if let Some(output) = plain_text_fallback {
        return Ok(output);
    }

    let fallback_text = html_to_text(html);
    if plain_text_looks_usable(&fallback_text) {
        return Ok(ReadableOutput {
            title: extract_html_title(html),
            content: fallback_text,
            content_representation: "plain_text",
            extractor: "html_to_text",
        });
    }

    Err(BitFunError::tool(
        "Failed to extract readable content from HTML".to_string(),
    ))
}

fn attempt_legible(html: &str, base_url: &str) -> BitFunResult<ExtractedCandidate> {
    let options = LegibleOptions::new().char_threshold(200);
    let article = match parse_legible(html, Some(base_url), Some(options)) {
        Ok(article) => article,
        Err(LegibleError::NoBody) => {
            let wrapped = wrap_html_in_body(html);
            parse_legible(
                &wrapped,
                Some(base_url),
                Some(LegibleOptions::new().char_threshold(200)),
            )
            .map_err(|err| BitFunError::tool(format!("Legible extraction failed: {}", err)))?
        }
        Err(err) => {
            return Err(BitFunError::tool(format!(
                "Legible extraction failed: {}",
                err
            )))
        }
    };

    let markdown = convert_html_to_markdown(&article.content, base_url).ok();

    Ok(ExtractedCandidate {
        title: non_empty_string(article.title).or_else(|| extract_html_title(html)),
        extractor: "legible",
        markdown,
        text: article.text_content,
    })
}

fn attempt_readability_js(html: &str, base_url: &str) -> BitFunResult<ExtractedCandidate> {
    let reader = Readability::new().map_err(|err| {
        BitFunError::tool(format!("Failed to initialize readability-js: {}", err))
    })?;
    let options = ReadabilityOptions::new().char_threshold(200);
    let article = reader
        .parse_with_options(html, Some(base_url), Some(options))
        .map_err(|err| BitFunError::tool(format!("readability-js extraction failed: {}", err)))?;

    let markdown = convert_html_to_markdown(&article.content, base_url).ok();

    Ok(ExtractedCandidate {
        title: non_empty_string(article.title).or_else(|| extract_html_title(html)),
        extractor: "readability_js",
        markdown,
        text: article.text_content,
    })
}

fn convert_html_to_markdown(html: &str, base_url: &str) -> BitFunResult<String> {
    let converter = HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "noscript", "iframe"])
        .build();
    let markdown = converter
        .convert(html)
        .map_err(|err| BitFunError::tool(format!("Failed to convert HTML to markdown: {}", err)))?;
    Ok(normalize_markdown(&absolutize_root_relative_markdown(
        &markdown, base_url,
    )))
}

fn absolutize_root_relative_markdown(markdown: &str, base_url: &str) -> String {
    let Some(origin) = origin_for(base_url) else {
        return markdown.to_string();
    };

    let pattern = Regex::new(r"\]\((/[^)\s]*)\)").expect("valid markdown link regex");
    pattern
        .replace_all(markdown, |captures: &Captures<'_>| {
            format!("]({}{})", origin, &captures[1])
        })
        .to_string()
}

fn origin_for(base_url: &str) -> Option<String> {
    let (scheme, rest) = base_url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    if host_port.is_empty() {
        return None;
    }
    Some(format!("{}://{}", scheme, host_port))
}

fn wrap_html_in_body(html: &str) -> String {
    if html.to_lowercase().contains("<body") {
        return html.to_string();
    }
    format!("<html><body>{}</body></html>", html)
}

pub(crate) fn extract_html_title(html: &str) -> Option<String> {
    let captures = Regex::new(r"(?is)<title[^>]*>(.*?)</title>")
        .expect("valid title regex")
        .captures(html)?;
    let title = captures.get(1)?.as_str();
    let title = normalize_text(&decode_basic_entities(title));
    non_empty_string(title)
}

pub(crate) fn html_to_text(html: &str) -> String {
    let mut text = html.to_string();
    for tag in [
        "script", "style", "noscript", "nav", "header", "footer", "aside", "iframe",
    ] {
        let pattern = format!(r"(?is)<{}[^>]*>[\s\S]*?</\s*{}\s*>", tag, tag);
        if let Ok(re) = Regex::new(&pattern) {
            text = re.replace_all(&text, "\n").to_string();
        }
    }

    let text = Regex::new(r"(?i)<br\s*/?>")
        .expect("valid br regex")
        .replace_all(&text, "\n");

    let text = Regex::new(r"<[^>]+>")
        .expect("valid tag regex")
        .replace_all(&text, " ");

    normalize_text(&decode_basic_entities(&text))
}

pub(crate) fn looks_noisy(content: &str) -> bool {
    NOISE_MARKERS.iter().any(|marker| content.contains(marker))
}

fn markdown_looks_usable(markdown: &str) -> bool {
    let normalized = normalize_markdown(markdown);
    normalized.chars().count() >= MIN_MARKDOWN_CHARS
        && !looks_noisy(&normalized)
        && !looks_like_html(&normalized)
}

fn plain_text_looks_usable(text: &str) -> bool {
    let normalized = normalize_text(text);
    normalized.chars().count() >= MIN_PLAIN_TEXT_CHARS
        && !looks_noisy(&normalized)
        && !looks_like_html(&normalized)
}

fn looks_like_html(content: &str) -> bool {
    let lower = content.to_lowercase();
    lower.contains("<html")
        || lower.contains("<body")
        || lower.contains("<script")
        || lower.contains("<div")
        || lower.contains("<!doctype html")
}

fn normalize_markdown(markdown: &str) -> String {
    let markdown = decode_basic_entities(markdown);
    let mut out = String::new();
    let mut blank_run = 0;

    for line in markdown.lines() {
        let trimmed_end = line.trim_end();
        if trimmed_end.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 2 {
                out.push('\n');
            }
            continue;
        }

        blank_run = 0;
        out.push_str(trimmed_end);
        out.push('\n');
    }

    out.trim().to_string()
}

fn normalize_text(text: &str) -> String {
    text.lines()
        .map(|line| {
            let mut result = String::new();
            let mut prev_space = true;
            for ch in line.chars() {
                if ch.is_whitespace() {
                    if !prev_space {
                        result.push(' ');
                        prev_space = true;
                    }
                } else {
                    result.push(ch);
                    prev_space = false;
                }
            }
            result.trim().to_string()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn decode_basic_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
}

fn non_empty_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{absolutize_root_relative_markdown, origin_for};

    #[test]
    fn origin_for_preserves_http_authority() {
        assert_eq!(
            origin_for("https://example.com:8443/docs/page?view=full#top"),
            Some("https://example.com:8443".to_string())
        );
    }

    #[test]
    fn origin_for_strips_userinfo_from_authority() {
        assert_eq!(
            origin_for("https://user:token@example.com/docs"),
            Some("https://example.com".to_string())
        );
    }

    #[test]
    fn absolutize_root_relative_markdown_uses_base_origin() {
        let markdown = "[Docs](/guide/start) [External](https://example.org)";

        assert_eq!(
            absolutize_root_relative_markdown(markdown, "https://bitfun.dev/docs/page"),
            "[Docs](https://bitfun.dev/guide/start) [External](https://example.org)"
        );
    }
}
