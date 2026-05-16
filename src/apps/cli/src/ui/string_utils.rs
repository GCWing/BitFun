/// String processing utilities

use unicode_width::UnicodeWidthChar;

/// Safely truncate string to specified byte length
pub fn truncate_str(s: &str, max_bytes: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    
    if first_line.len() <= max_bytes {
        return first_line.to_string();
    }
    
    let mut boundary = max_bytes;
    while boundary > 0 && !first_line.is_char_boundary(boundary) {
        boundary -= 1;
    }
    
    if boundary == 0 {
        return String::new();
    }
    
    format!("{}...", &first_line[..boundary])
}

/// Strip ANSI escape sequences from a string.
/// Handles CSI sequences (\x1b[...X), OSC sequences (\x1b]...ST), and simple two-byte escapes.
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // ESC character — consume the escape sequence
            match chars.peek() {
                Some('[') => {
                    // CSI sequence: ESC [ ... (ends at 0x40-0x7E)
                    chars.next(); // consume '['
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c.is_ascii() && (0x40..=0x7E).contains(&(c as u8)) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC sequence: ESC ] ... (ends at BEL \x07 or ST \x1b\\)
                    chars.next(); // consume ']'
                    while let Some(&c) = chars.peek() {
                        if c == '\x07' {
                            chars.next();
                            break;
                        }
                        if c == '\x1b' {
                            chars.next();
                            if chars.peek() == Some(&'\\') {
                                chars.next();
                            }
                            break;
                        }
                        chars.next();
                    }
                }
                Some(_) => {
                    // Simple two-byte escape (e.g. ESC M, ESC 7, ESC 8)
                    chars.next();
                }
                None => {}
            }
        } else if ch == '\r' {
            // Skip carriage return (common in terminal output)
            continue;
        } else {
            result.push(ch);
        }
    }

    result
}

/// Hard-wrap a single line to fit within display width (columns).
/// Preserves all characters (no truncation), splitting long lines into multiple lines.
pub fn wrap_to_display_width(s: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![String::new()];
    }
    if s.is_empty() {
        return vec![String::new()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);

        if !current.is_empty() && current_width + ch_width > max_width {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }

        current.push(ch);
        current_width += ch_width;

        if current_width >= max_width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

