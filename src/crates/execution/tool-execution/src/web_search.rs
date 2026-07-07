#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub fn parse_exa_text_results(text: &str) -> Vec<WebSearchResult> {
    let mut out = Vec::new();
    let mut cur: Option<(String, String, Vec<String>)> = None;
    let mut body = false;

    for line in text.lines() {
        if let Some(next) = line.strip_prefix("Title: ") {
            if let Some((title, url, text)) = cur.take() {
                out.push(item(title, url, text));
            }
            cur = Some((next.trim().to_string(), String::new(), Vec::new()));
            body = false;
            continue;
        }

        let Some(cur) = cur.as_mut() else {
            continue;
        };

        if let Some(next) = line.strip_prefix("URL: ") {
            cur.1 = next.trim().to_string();
            continue;
        }

        if let Some(next) = line.strip_prefix("Text: ") {
            if !next.trim().is_empty() {
                cur.2.push(next.trim().to_string());
            }
            body = true;
            continue;
        }

        if body {
            cur.2.push(line.to_string());
        }
    }

    if let Some((title, url, text)) = cur.take() {
        out.push(item(title, url, text));
    }

    if out.is_empty() && !text.trim().is_empty() {
        return vec![WebSearchResult {
            title: "Web search result".to_string(),
            url: String::new(),
            snippet: snippet(text),
        }];
    }

    out
}

fn item(title: String, url: String, text: Vec<String>) -> WebSearchResult {
    WebSearchResult {
        title,
        url,
        snippet: snippet(&text.join("\n")),
    }
}

fn snippet(text: &str) -> String {
    let text = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join(" ");

    if text.chars().count() <= 320 {
        return text;
    }

    let mut out = String::new();
    for ch in text.chars().take(317) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::parse_exa_text_results;

    #[test]
    fn parses_exa_text_blocks() {
        let results = parse_exa_text_results(
            "Title: First\nURL: https://example.com/a\nText: # Heading\nUseful line\n\nTitle: Second\nURL: https://example.com/b\nText: Other line",
        );

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "First");
        assert_eq!(results[0].url, "https://example.com/a");
        assert_eq!(results[0].snippet, "Useful line");
        assert_eq!(results[1].title, "Second");
    }

    #[test]
    fn falls_back_for_unstructured_text() {
        let results = parse_exa_text_results("one\n\n# heading\nbody");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Web search result");
        assert_eq!(results[0].url, "");
        assert_eq!(results[0].snippet, "one body");
    }

    #[test]
    fn truncates_snippet_on_char_boundary() {
        let text = "Title: Long\nURL: https://example.com\nText: ".to_string() + &"你".repeat(400);
        let results = parse_exa_text_results(&text);

        assert_eq!(results[0].snippet.chars().count(), 320);
        assert!(results[0].snippet.ends_with("..."));
    }
}
