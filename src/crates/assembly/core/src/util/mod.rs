//! Common utilities and type definitions

pub mod errors;
#[cfg(feature = "workspace-runtime")]
pub mod front_matter_markdown;
pub mod json_extract;
pub mod plain_output;
#[cfg(feature = "process-runtime")]
pub use openbitfun_services_core::process_manager;
pub mod timing;
pub mod token_counter;
pub mod types;

pub use errors::*;
#[cfg(feature = "workspace-runtime")]
pub use front_matter_markdown::FrontMatterMarkdown;
pub use json_extract::extract_json_from_ai_response;
pub use plain_output::sanitize_plain_model_output;
#[cfg(feature = "process-runtime")]
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
pub(crate) fn create_test_command(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let command = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        let mut command = command;
        command.creation_flags(0x0800_0000);
        command
    }
    #[cfg(not(windows))]
    command
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
