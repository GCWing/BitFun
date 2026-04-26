//! Common utilities and type definitions

pub mod errors;
pub mod front_matter_markdown;
pub mod json_extract;
pub mod plain_output;
pub mod process_manager;
pub mod timing;
pub mod token_counter;
pub mod types;

pub use errors::*;
pub use front_matter_markdown::FrontMatterMarkdown;
pub use json_extract::extract_json_from_ai_response;
pub use plain_output::sanitize_plain_model_output;
pub use process_manager::*;
pub use timing::*;
pub use token_counter::*;
pub use types::*;

pub fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::truncate_at_char_boundary;

    #[test]
    fn truncate_at_char_boundary_keeps_ascii_prefix() {
        assert_eq!(truncate_at_char_boundary("abcdef", 3), "abc");
    }

    #[test]
    fn truncate_at_char_boundary_backs_up_from_multibyte_character() {
        let text = format!("{}{}", "a".repeat(62), "案".repeat(2));

        assert_eq!(truncate_at_char_boundary(&text, 64), "a".repeat(62));
    }

    #[test]
    fn truncate_at_char_boundary_returns_full_text_when_short_enough() {
        assert_eq!(truncate_at_char_boundary("短文本", 64), "短文本");
    }
}
