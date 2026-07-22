use crate::chars;

use super::JsonRepairer;
use super::Result;

impl JsonRepairer {
    pub(super) fn parse_markdown_fenced(&mut self) -> Result<bool> {
        if !self.matches_at(self.pos, "```") {
            return Ok(false);
        }

        self.pos += 3; // skip opening ```

        // Optional language tag: ```json
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                self.pos += 1;
            } else {
                break;
            }
        }

        self.parse_whitespace_and_comments();

        let processed_value = self.parse_value()?;
        if !processed_value {
            return Ok(false);
        }

        self.parse_whitespace_and_comments();

        if self.matches_at(self.pos, "```") {
            self.pos += 3;
        }

        Ok(true)
    }

    pub(super) fn parse_markdown_wrapped_open(&mut self) -> bool {
        self.parse_whitespace_and_comments();
        for block in ["```", "[```", "{```"] {
            if self.matches_at(self.pos, block) {
                self.pos += block.len();

                // Optional language tag like ```json
                while self.peek().is_some_and(chars::is_identifier_char) {
                    self.pos += 1;
                }

                self.parse_whitespace_and_comments();
                return true;
            }
        }
        false
    }

    pub(super) fn parse_markdown_wrapped_close(&mut self) -> bool {
        self.parse_whitespace_and_comments();
        for block in ["```", "```]", "```}"] {
            if self.matches_at(self.pos, block) {
                self.pos += block.len();
                self.parse_whitespace_and_comments();
                return true;
            }
        }
        false
    }

    pub(super) fn parse_regex_as_string(&mut self) -> Result<bool> {
        if self.peek() != Some('/') {
            return Ok(false);
        }
        self.pos += 1;
        self.output.push('"');
        self.output.push('/');
        let mut escaped = false;

        loop {
            match self.peek() {
                None | Some('\n') | Some('\r') => {
                    self.output.push('/');
                    self.output.push('"');
                    return Ok(true);
                }
                Some(c) => {
                    self.pos += 1;
                    if c == '/' && !escaped {
                        self.output.push('/');
                        while self.peek().is_some_and(|flag| flag.is_ascii_alphabetic()) {
                            let flag = self.chars[self.pos];
                            self.push_string_char(flag);
                            self.pos += 1;
                        }
                        self.output.push('"');
                        return Ok(true);
                    }

                    self.push_string_char(c);
                    escaped = c == '\\' && !escaped;
                }
            }
        }
    }
}
