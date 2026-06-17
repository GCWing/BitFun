use std::mem;

const INLINE_THINK_OPEN_TAG: &str = "<think>";
const INLINE_THINK_CLOSE_TAG: &str = "</think>";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activation {
    Unknown,
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Text,
    Thinking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InlineThinkSegment {
    Text(String),
    Thinking(String),
}

#[derive(Debug)]
pub(crate) struct InlineThinkRouter {
    activation: Activation,
    mode: Mode,
    pending_tail: String,
    initial_probe: String,
}

impl InlineThinkRouter {
    pub(crate) fn new() -> Self {
        Self {
            activation: Activation::Unknown,
            mode: Mode::Text,
            pending_tail: String::new(),
            initial_probe: String::new(),
        }
    }

    pub(crate) fn route_text(&mut self, text: String) -> Vec<InlineThinkSegment> {
        match self.activation {
            Activation::Unknown => self.consume_unknown_text(text),
            Activation::Enabled => self.parse_enabled_text(text),
            Activation::Disabled => vec![InlineThinkSegment::Text(text)],
        }
    }

    pub(crate) fn flush(&mut self) -> Vec<InlineThinkSegment> {
        match self.activation {
            Activation::Unknown => {
                let pending = mem::take(&mut self.initial_probe);
                if pending.is_empty() {
                    Vec::new()
                } else {
                    vec![InlineThinkSegment::Text(pending)]
                }
            }
            Activation::Enabled => {
                let pending = mem::take(&mut self.pending_tail);
                if pending.is_empty() {
                    Vec::new()
                } else if self.mode == Mode::Thinking {
                    vec![InlineThinkSegment::Thinking(pending)]
                } else {
                    vec![InlineThinkSegment::Text(pending)]
                }
            }
            Activation::Disabled => Vec::new(),
        }
    }

    fn consume_unknown_text(&mut self, text: String) -> Vec<InlineThinkSegment> {
        self.initial_probe.push_str(&text);

        let trimmed = self.initial_probe.trim_start_matches(char::is_whitespace);
        if trimmed.is_empty() {
            return Vec::new();
        }

        if trimmed.starts_with(INLINE_THINK_OPEN_TAG) {
            self.activation = Activation::Enabled;
            let buffered = mem::take(&mut self.initial_probe);
            return self.parse_enabled_text(buffered);
        }

        if INLINE_THINK_OPEN_TAG.starts_with(trimmed) {
            return Vec::new();
        }

        self.activation = Activation::Disabled;
        vec![InlineThinkSegment::Text(mem::take(&mut self.initial_probe))]
    }

    fn parse_enabled_text(&mut self, text: String) -> Vec<InlineThinkSegment> {
        let mut data = mem::take(&mut self.pending_tail);
        data.push_str(&text);

        let mut segments = Vec::new();

        loop {
            let marker = match self.mode {
                Mode::Text => INLINE_THINK_OPEN_TAG,
                Mode::Thinking => INLINE_THINK_CLOSE_TAG,
            };

            if let Some(marker_idx) = data.find(marker) {
                let before_marker = data[..marker_idx].to_string();
                self.push_segment(&mut segments, before_marker);

                data = data[marker_idx + marker.len()..].to_string();
                self.mode = match self.mode {
                    Mode::Text => Mode::Thinking,
                    Mode::Thinking => Mode::Text,
                };
                continue;
            }

            let tail_len = longest_suffix_prefix_len(&data, marker);
            let flush_len = data.len() - tail_len;
            let ready = data[..flush_len].to_string();
            self.push_segment(&mut segments, ready);
            self.pending_tail = data[flush_len..].to_string();
            break;
        }

        segments
    }

    fn push_segment(&self, segments: &mut Vec<InlineThinkSegment>, content: String) {
        if content.is_empty() {
            return;
        }

        match self.mode {
            Mode::Text => segments.push(InlineThinkSegment::Text(content)),
            Mode::Thinking => segments.push(InlineThinkSegment::Thinking(content)),
        }
    }
}

fn longest_suffix_prefix_len(value: &str, marker: &str) -> usize {
    let max_len = value.len().min(marker.len().saturating_sub(1));
    (1..=max_len)
        .rev()
        .find(|&len| value.ends_with(&marker[..len]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{InlineThinkRouter, InlineThinkSegment};

    #[test]
    fn routes_initial_inline_thinking_to_thought_segments() {
        let mut router = InlineThinkRouter::new();

        let first = router.route_text("<think>abc".to_string());
        let second = router.route_text("def</think>ghi".to_string());

        assert_eq!(first, vec![InlineThinkSegment::Thinking("abc".to_string())]);
        assert_eq!(
            second,
            vec![
                InlineThinkSegment::Thinking("def".to_string()),
                InlineThinkSegment::Text("ghi".to_string())
            ]
        );
    }

    #[test]
    fn handles_split_opening_tag() {
        let mut router = InlineThinkRouter::new();

        assert!(router.route_text("<thi".to_string()).is_empty());
        assert_eq!(
            router.route_text("nk>hidden</think>visible".to_string()),
            vec![
                InlineThinkSegment::Thinking("hidden".to_string()),
                InlineThinkSegment::Text("visible".to_string())
            ]
        );
    }

    #[test]
    fn leaves_non_initial_tags_as_message_text() {
        let mut router = InlineThinkRouter::new();

        assert_eq!(
            router.route_text("hello <think>literal".to_string()),
            vec![InlineThinkSegment::Text("hello <think>literal".to_string())]
        );
        assert_eq!(
            router.route_text("</think> world".to_string()),
            vec![InlineThinkSegment::Text("</think> world".to_string())]
        );
    }

    #[test]
    fn flushes_unclosed_thinking_without_tags() {
        let mut router = InlineThinkRouter::new();

        assert_eq!(
            router.route_text("<think>abc".to_string()),
            vec![InlineThinkSegment::Thinking("abc".to_string())]
        );
        assert!(router.flush().is_empty());
    }

    #[test]
    fn flushes_unknown_probe_as_text() {
        let mut router = InlineThinkRouter::new();

        assert!(router.route_text("   ".to_string()).is_empty());
        assert_eq!(
            router.flush(),
            vec![InlineThinkSegment::Text("   ".to_string())]
        );
    }
}
