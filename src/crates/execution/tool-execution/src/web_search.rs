#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub published: String,
    pub author: String,
}

pub fn parse_exa_text_results(text: &str) -> Vec<WebSearchResult> {
    let mut out = Vec::new();
    let mut cur: Option<WebSearchResult> = None;

    for line in text.lines() {
        if let Some(next) = line.strip_prefix("Title: ") {
            if let Some(result) = cur.take() {
                out.push(result);
            }
            cur = Some(WebSearchResult {
                title: next.trim().to_string(),
                url: String::new(),
                published: String::new(),
                author: String::new(),
            });
            continue;
        }

        let Some(cur) = cur.as_mut() else {
            continue;
        };

        if let Some(next) = line.strip_prefix("URL: ") {
            cur.url = next.trim().to_string();
            continue;
        }

        if let Some(next) = line
            .strip_prefix("Published: ")
            .or_else(|| line.strip_prefix("Published Date: "))
        {
            cur.published = next.trim().to_string();
            continue;
        }

        if let Some(next) = line.strip_prefix("Author: ") {
            cur.author = next.trim().to_string();
        }
    }

    if let Some(result) = cur {
        out.push(result);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::parse_exa_text_results;

    #[test]
    fn parses_exa_text_blocks() {
        let results = parse_exa_text_results(
            "Title: First\nURL: https://example.com/a\nPublished: 2026-08-30T00:00:00.000Z\nAuthor: First Author\nHighlights:\nVery long content that must not be retained.\n\nTitle: Second\nURL: https://example.com/b\nPublished Date: N/A\nAuthor: N/A\nHighlights:\nOther long content.",
        );

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "First");
        assert_eq!(results[0].url, "https://example.com/a");
        assert_eq!(results[0].published, "2026-08-30T00:00:00.000Z");
        assert_eq!(results[0].author, "First Author");
        assert_eq!(results[1].title, "Second");
        assert_eq!(results[1].published, "N/A");
        assert_eq!(results[1].author, "N/A");
    }

    #[test]
    fn ignores_unstructured_text_instead_of_returning_it_as_context() {
        let results = parse_exa_text_results("one\n\n# heading\nbody");

        assert!(results.is_empty());
    }
}
