#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenTextTag {
    pub name: String,
    pub open: String,
    pub close: String,
}

impl HiddenTextTag {
    pub fn new(name: impl Into<String>, open: impl Into<String>, close: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            open: open.into(),
            close: close.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HiddenTextBlock {
    pub name: String,
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HiddenTextChunk {
    pub visible_text: String,
    pub hidden_blocks: Vec<HiddenTextBlock>,
}

impl HiddenTextChunk {
    pub fn is_empty(&self) -> bool {
        self.visible_text.is_empty() && self.hidden_blocks.is_empty()
    }
}

#[derive(Debug)]
pub struct HiddenTextStreamParser {
    tags: Vec<HiddenTextTag>,
    state: ParserState,
    buffer: String,
}

#[derive(Debug)]
enum ParserState {
    Visible,
    Hidden { tag_index: usize },
}

impl HiddenTextStreamParser {
    pub fn new(tags: Vec<HiddenTextTag>) -> Self {
        Self {
            tags: tags
                .into_iter()
                .filter(|tag| !tag.open.is_empty() && !tag.close.is_empty())
                .collect(),
            state: ParserState::Visible,
            buffer: String::new(),
        }
    }

    pub fn push_str(&mut self, chunk: &str) -> HiddenTextChunk {
        if self.tags.is_empty() {
            return HiddenTextChunk {
                visible_text: chunk.to_string(),
                hidden_blocks: Vec::new(),
            };
        }

        self.buffer.push_str(chunk);
        self.drain(false)
    }

    pub fn finish(&mut self) -> HiddenTextChunk {
        if self.tags.is_empty() {
            let visible_text = std::mem::take(&mut self.buffer);
            return HiddenTextChunk {
                visible_text,
                hidden_blocks: Vec::new(),
            };
        }

        self.drain(true)
    }

    fn drain(&mut self, finish: bool) -> HiddenTextChunk {
        let mut out = HiddenTextChunk::default();

        loop {
            match self.state {
                ParserState::Visible => {
                    if let Some((start, tag_index)) = self.find_next_open_tag() {
                        out.visible_text.push_str(&self.buffer[..start]);
                        let open_len = self.tags[tag_index].open.len();
                        self.buffer.drain(..start + open_len);
                        self.state = ParserState::Hidden { tag_index };
                        continue;
                    }

                    let keep = if finish {
                        0
                    } else {
                        longest_tag_prefix_suffix_len(&self.buffer, &self.tags)
                    };
                    let emit_len = self.buffer.len().saturating_sub(keep);
                    if emit_len > 0 {
                        out.visible_text.push_str(&self.buffer[..emit_len]);
                        self.buffer.drain(..emit_len);
                    }
                    break;
                }
                ParserState::Hidden { tag_index } => {
                    let tag = &self.tags[tag_index];
                    if let Some(end) = self.buffer.find(&tag.close) {
                        out.hidden_blocks.push(HiddenTextBlock {
                            name: tag.name.clone(),
                            payload: self.buffer[..end].to_string(),
                        });
                        self.buffer.drain(..end + tag.close.len());
                        self.state = ParserState::Visible;
                        continue;
                    }

                    if finish {
                        out.hidden_blocks.push(HiddenTextBlock {
                            name: tag.name.clone(),
                            payload: std::mem::take(&mut self.buffer),
                        });
                        self.state = ParserState::Visible;
                    }
                    break;
                }
            }
        }

        out
    }

    fn find_next_open_tag(&self) -> Option<(usize, usize)> {
        self.tags
            .iter()
            .enumerate()
            .filter_map(|(index, tag)| self.buffer.find(&tag.open).map(|start| (start, index)))
            .min_by_key(|(start, _)| *start)
    }
}

fn longest_tag_prefix_suffix_len(text: &str, tags: &[HiddenTextTag]) -> usize {
    tags.iter()
        .filter(|tag| !tag.open.is_empty())
        .flat_map(|tag| {
            (1..tag.open.len()).filter(move |&len| {
                if text.len() < len || !text.is_char_boundary(text.len() - len) {
                    return false;
                }
                let suffix = &text[text.len() - len..];
                tag.open.starts_with(suffix)
            })
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{HiddenTextStreamParser, HiddenTextTag};

    fn memory_tag() -> HiddenTextTag {
        HiddenTextTag::new(
            "memory_citation",
            "<bitfun-mem-citation>",
            "</bitfun-mem-citation>",
        )
    }

    #[test]
    fn streams_hidden_tag_across_chunk_boundaries() {
        let mut parser = HiddenTextStreamParser::new(vec![memory_tag()]);

        let first = parser.push_str("hello <bitfun-mem-");
        let second = parser.push_str("citation>doc</bitfun-mem-citation> world");
        let tail = parser.finish();

        assert_eq!(first.visible_text, "hello ");
        assert!(first.hidden_blocks.is_empty());
        assert_eq!(second.visible_text, " world");
        assert_eq!(second.hidden_blocks.len(), 1);
        assert_eq!(second.hidden_blocks[0].name, "memory_citation");
        assert_eq!(second.hidden_blocks[0].payload, "doc");
        assert!(tail.is_empty());
    }

    #[test]
    fn auto_closes_unterminated_hidden_tag_at_finish() {
        let mut parser = HiddenTextStreamParser::new(vec![memory_tag()]);

        let first = parser.push_str("x<bitfun-mem-citation>doc");
        let tail = parser.finish();

        assert_eq!(first.visible_text, "x");
        assert!(first.hidden_blocks.is_empty());
        assert_eq!(tail.visible_text, "");
        assert_eq!(tail.hidden_blocks[0].payload, "doc");
    }

    #[test]
    fn preserves_partial_open_tag_at_finish_when_not_complete() {
        let mut parser = HiddenTextStreamParser::new(vec![memory_tag()]);

        let first = parser.push_str("hello <bitfun-mem-");
        let tail = parser.finish();

        assert_eq!(first.visible_text, "hello ");
        assert_eq!(tail.visible_text, "<bitfun-mem-");
        assert!(tail.hidden_blocks.is_empty());
    }

    #[test]
    fn ignores_tags_with_empty_delimiters() {
        let mut parser = HiddenTextStreamParser::new(vec![
            HiddenTextTag::new("empty_open", "", "</x>"),
            HiddenTextTag::new("empty_close", "<x>", ""),
        ]);

        let first = parser.push_str("a<x>b</x>c");
        let tail = parser.finish();

        assert_eq!(first.visible_text, "a<x>b</x>c");
        assert!(first.hidden_blocks.is_empty());
        assert!(tail.is_empty());
    }
}
