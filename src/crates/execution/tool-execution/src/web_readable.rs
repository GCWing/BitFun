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

pub fn normalize_requested_format(format: Option<&str>) -> Result<RequestedWebFetchFormat, String> {
    match format.unwrap_or("markdown") {
        "raw" => Ok(RequestedWebFetchFormat::Raw),
        "markdown" | "text" => Ok(RequestedWebFetchFormat::Markdown),
        "json" => Ok(RequestedWebFetchFormat::Json),
        other => Err(format!(
            "Unsupported format '{}'. Expected raw, markdown, or json.",
            other
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedWebFetchFormat {
    Raw,
    Markdown,
    Json,
}

#[derive(Debug, Clone)]
pub struct ReadableWebOutput {
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

type ExtractorFn = fn(&str, &str) -> Result<ExtractedCandidate, String>;

pub fn is_html(content_type: Option<&str>, content: &str) -> bool {
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

pub fn extract_markdown_with_text_fallback(
    html: &str,
    base_url: &str,
) -> Result<ReadableWebOutput, String> {
    let mut plain_text_fallback: Option<ReadableWebOutput> = None;

    // Keep the existing extractor order. Local extraction experiments across
    // article, documentation, wiki, and forum pages showed `legible` gives the
    // best current quality/latency balance, with readability-js as fallback.
    for extractor in [
        attempt_legible as ExtractorFn,
        attempt_readability_js as ExtractorFn,
    ] {
        let Ok(candidate) = extractor(html, base_url) else {
            continue;
        };

        if let Some(markdown) = candidate.markdown {
            if markdown_looks_usable(&markdown) {
                return Ok(ReadableWebOutput {
                    title: candidate.title,
                    content: markdown,
                    content_representation: "markdown",
                    extractor: candidate.extractor,
                });
            }
        }

        if plain_text_fallback.is_none() && plain_text_looks_usable(&candidate.text) {
            plain_text_fallback = Some(ReadableWebOutput {
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
        return Ok(ReadableWebOutput {
            title: extract_html_title(html),
            content: fallback_text,
            content_representation: "plain_text",
            extractor: "html_to_text",
        });
    }

    Err("Failed to extract readable content from HTML".to_string())
}

fn attempt_legible(html: &str, base_url: &str) -> Result<ExtractedCandidate, String> {
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
            .map_err(|err| format!("Legible extraction failed: {}", err))?
        }
        Err(err) => return Err(format!("Legible extraction failed: {}", err)),
    };

    let markdown = convert_html_to_markdown(&article.content, base_url).ok();

    Ok(ExtractedCandidate {
        title: non_empty_string(article.title).or_else(|| extract_html_title(html)),
        extractor: "legible",
        markdown,
        text: article.text_content,
    })
}

fn attempt_readability_js(html: &str, base_url: &str) -> Result<ExtractedCandidate, String> {
    let reader = Readability::new()
        .map_err(|err| format!("Failed to initialize readability-js: {}", err))?;
    let options = ReadabilityOptions::new().char_threshold(200);
    let article = reader
        .parse_with_options(html, Some(base_url), Some(options))
        .map_err(|err| format!("readability-js extraction failed: {}", err))?;

    let markdown = convert_html_to_markdown(&article.content, base_url).ok();

    Ok(ExtractedCandidate {
        title: non_empty_string(article.title).or_else(|| extract_html_title(html)),
        extractor: "readability_js",
        markdown,
        text: article.text_content,
    })
}

fn convert_html_to_markdown(html: &str, base_url: &str) -> Result<String, String> {
    let converter = HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "noscript", "iframe"])
        .build();
    let markdown = converter
        .convert(html)
        .map_err(|err| format!("Failed to convert HTML to markdown: {}", err))?;
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

pub fn extract_html_title(html: &str) -> Option<String> {
    let captures = Regex::new(r"(?is)<title[^>]*>(.*?)</title>")
        .expect("valid title regex")
        .captures(html)?;
    let title = captures.get(1)?.as_str();
    let title = normalize_text(&decode_basic_entities(title));
    non_empty_string(title)
}

pub fn html_to_text(html: &str) -> String {
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

pub fn looks_noisy(content: &str) -> bool {
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

fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}

#[cfg(test)]
mod tests {
    use super::{
        absolutize_root_relative_markdown, extract_html_title, extract_markdown_with_text_fallback,
        html_to_text, is_html, looks_noisy, normalize_requested_format, origin_for,
        RequestedWebFetchFormat,
    };

    #[test]
    fn webfetch_text_alias_normalizes_to_markdown() {
        assert!(matches!(
            normalize_requested_format(Some("text")).expect("format alias should work"),
            RequestedWebFetchFormat::Markdown
        ));
    }

    #[test]
    fn html_to_text_extracts_plain_text() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test Page</title></head>
<body>
<script>alert('ignore me');</script>
<style>.hidden { display: none; }</style>
<h1>Hello World</h1>
<p>This is a paragraph with <strong>bold</strong> text.</p>
<ul><li>Item one</li><li>Item two</li></ul>
</body>
</html>"#;

        let text = html_to_text(html);
        assert!(!text.contains("<script>"));
        assert!(!text.contains("alert("));
        assert!(!text.contains(".hidden"));
        assert!(text.contains("Hello World"));
        assert!(text.contains("This is a paragraph with bold text."));
        assert!(text.contains("Item one"));
        assert!(text.contains("Item two"));
    }

    #[test]
    fn is_html_detects_html_content() {
        assert!(is_html(Some("text/html; charset=utf-8"), "any"));
        assert!(is_html(Some("application/xhtml+xml"), "any"));
        assert!(is_html(None, "<!DOCTYPE html><html></html>"));
        assert!(is_html(None, "<html lang=\"en\"></html>"));
        assert!(!is_html(Some("application/json"), "{}"));
        assert!(!is_html(Some("text/plain"), "hello"));
        assert!(!is_html(None, "just plain text"));
    }

    #[test]
    fn detects_noisy_markdown() {
        assert!(looks_noisy(
            "header __next_f.push([1,2,3]) siteSettings footer"
        ));
        assert!(!looks_noisy("# Hello\n\nThis is a clean article."));
    }

    #[test]
    fn extracts_markdown_for_simple_html() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Hello World</title></head>
<body>
  <article>
    <h1>Hello World</h1>
    <p>This is the primary article content.</p>
    <p>It should become readable markdown.</p>
  </article>
  <footer>Ignore this footer</footer>
</body>
</html>"#;

        let result = extract_markdown_with_text_fallback(html, "https://example.com/article")
            .expect("readable extraction should succeed");
        assert_eq!(result.content_representation, "markdown");
        assert_eq!(result.title.as_deref(), Some("Hello World"));
        assert!(result.content.contains("primary article content"));
        assert!(!result.content.contains("Ignore this footer"));
    }

    #[test]
    fn extracts_html_title() {
        let html =
            r#"<html><head><title>Example Title</title></head><body><p>Hello</p></body></html>"#;
        assert_eq!(extract_html_title(html).as_deref(), Some("Example Title"));
    }

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
