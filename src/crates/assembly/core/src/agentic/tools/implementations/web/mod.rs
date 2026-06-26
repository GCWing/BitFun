//! Web tool implementations.

mod fetch;
mod readable;
mod search;

pub use fetch::WebFetchTool;
pub use search::WebSearchTool;

#[cfg(test)]
mod tests {
    use super::fetch::WebFetchTool;
    use super::readable::{
        extract_html_title, extract_markdown_with_text_fallback, html_to_text, is_html,
        looks_noisy, normalize_requested_format, RequestedFormat,
    };
    use super::search::WebSearchTool;
    use crate::agentic::tools::framework::{Tool, ToolResult, ToolUseContext};
    use serde_json::json;
    use std::io::ErrorKind;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn empty_context() -> ToolUseContext {
        ToolUseContext {
            tool_call_id: None,
            agent_type: None,
            session_id: None,
            dialog_turn_id: None,
            workspace: None,
            unlocked_collapsed_tools: Vec::new(),
            primary_model_facts: tool_runtime::context::PrimaryModelFacts::default(),
            custom_data: std::collections::HashMap::new(),
            computer_use_host: None,
            runtime_tool_restrictions: Default::default(),
            runtime_handles: bitfun_runtime_ports::ToolRuntimeHandles::default(),
        }
    }

    #[tokio::test]
    async fn webfetch_can_fetch_local_http_content() {
        let listener = match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(e) if e.kind() == ErrorKind::PermissionDenied => {
                eprintln!(
                    "Skipping webfetch local server test due to sandbox socket restrictions: {}",
                    e
                );
                return;
            }
            Err(e) => panic!("bind local test server: {}", e),
        };
        let addr = listener.local_addr().expect("read local addr");

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept request");
            let mut req_buf = [0u8; 1024];
            let _ = socket.read(&mut req_buf).await;

            let body = "hello from webfetch";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write response");
            let _ = socket.shutdown().await;
        });

        let tool = WebFetchTool::new();
        let input = json!({
            "url": format!("http://{}/test", addr),
            "format": "markdown"
        });

        let results = tool
            .call(&input, &empty_context())
            .await
            .unwrap_or_else(|e| {
                panic!("tool call failed with detailed error: {:?}", e);
            });
        assert_eq!(results.len(), 1);

        match &results[0] {
            ToolResult::Result {
                data,
                result_for_assistant,
                ..
            } => {
                assert_eq!(data["content"], "hello from webfetch");
                assert_eq!(data["format"], "markdown");
                assert_eq!(data["content_representation"], "plain_text");
                assert!(data["title"].is_null());
                assert_eq!(result_for_assistant.as_deref(), Some("hello from webfetch"));
            }
            other => panic!("unexpected tool result variant: {:?}", other),
        }

        server.await.expect("server task");
    }

    #[test]
    fn webfetch_text_alias_normalizes_to_markdown() {
        assert!(matches!(
            normalize_requested_format(Some("text")).expect("format alias should work"),
            RequestedFormat::Markdown
        ));
    }

    #[test]
    fn webfetch_html_to_text_extracts_plain_text() {
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
    fn webfetch_is_html_detects_html_content() {
        assert!(is_html(Some("text/html; charset=utf-8"), "any"));
        assert!(is_html(Some("application/xhtml+xml"), "any"));
        assert!(is_html(None, "<!DOCTYPE html><html></html>"));
        assert!(is_html(None, "<html lang=\"en\"></html>"));
        assert!(!is_html(Some("application/json"), "{}"));
        assert!(!is_html(Some("text/plain"), "hello"));
        assert!(!is_html(None, "just plain text"));
    }

    #[test]
    fn webfetch_detects_noisy_markdown() {
        assert!(looks_noisy(
            "header __next_f.push([1,2,3]) siteSettings footer"
        ));
        assert!(!looks_noisy("# Hello\n\nThis is a clean article."));
    }

    #[test]
    fn webfetch_extracts_markdown_for_simple_html() {
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
    fn webfetch_extracts_html_title() {
        let html =
            r#"<html><head><title>Example Title</title></head><body><p>Hello</p></body></html>"#;
        assert_eq!(extract_html_title(html).as_deref(), Some("Example Title"));
    }

    #[test]
    fn websearch_parses_exa_text_into_results() {
        let tool = WebSearchTool::new();
        let text = r#"Title: Result One
URL: https://example.com/one
Text: Result One

First paragraph.

Title: Result Two
URL: https://example.com/two
Text: Result Two

Second paragraph.
"#;

        let out = tool.results(text);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["title"], "Result One");
        assert_eq!(out[0]["url"], "https://example.com/one");
        assert_eq!(out[0]["snippet"], "Result One First paragraph.");
        assert_eq!(out[1]["title"], "Result Two");
    }
}
