//! Character classification utilities for JSON repair parsing.

/// Check if a character is a JSON digit (0-9).
#[inline]
pub(crate) fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// Check if a character is a hex digit (0-9, a-f, A-F).
#[inline]
pub(crate) fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

/// Check if a character is valid as the start of a JSON number.
#[inline]
pub(crate) fn is_number_start(c: char) -> bool {
    is_digit(c) || c == '-'
}

/// Check if a character is a quote (regular, single, or curly).
#[inline]
pub(crate) fn is_quote(c: char) -> bool {
    matches!(
        c,
        '"' | '\'' | '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}' | '\u{0060}' | '\u{00B4}'
    )
}

/// Check if a character is a double-quote-like character (regular or curly).
#[inline]
pub(crate) fn is_double_quote_like(c: char) -> bool {
    matches!(c, '"' | '\u{201C}' | '\u{201D}')
}

/// Check if a character is a plain double quote.
#[inline]
pub(crate) fn is_double_quote(c: char) -> bool {
    c == '"'
}

/// Check if a character is a single-quote-like character (regular, curly, or backtick).
#[inline]
pub(crate) fn is_single_quote_like(c: char) -> bool {
    matches!(c, '\'' | '\u{2018}' | '\u{2019}' | '\u{0060}' | '\u{00B4}')
}

/// Check if a character is a plain single quote.
#[inline]
pub(crate) fn is_single_quote(c: char) -> bool {
    c == '\''
}

/// Check if a character ends an unquoted string.
#[inline]
pub(crate) fn is_unquoted_string_delimiter(c: char) -> bool {
    matches!(c, ',' | '[' | ']' | '/' | '{' | '}' | '\n' | '+' | ';')
}

/// Check if a character is a generic JSON delimiter.
#[inline]
pub(crate) fn is_delimiter(c: char) -> bool {
    matches!(
        c,
        ',' | ':' | '[' | ']' | '/' | '{' | '}' | '(' | ')' | '\n' | '+' | ';'
    )
}

/// Check whether a character can start a JSON value.
#[inline]
pub(crate) fn is_start_of_value(c: char) -> bool {
    is_quote(c) || matches!(c, '[' | '{' | '-' | '+' | '.' | '_') || c.is_ascii_alphanumeric()
}

/// Characters that can occur in a URL.
#[inline]
pub(crate) fn is_url_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '-' | '.'
                | '_'
                | '~'
                | ':'
                | '/'
                | '?'
                | '#'
                | '@'
                | '!'
                | '$'
                | '&'
                | '\''
                | '('
                | ')'
                | '*'
                | '+'
                | ';'
                | '='
        )
}

/// Check if a string character is valid in JSON (must be >= U+0020).
#[inline]
pub(crate) fn is_valid_string_character(c: char) -> bool {
    c >= '\u{0020}'
}

/// Check if a character is a "special" (non-ASCII) whitespace.
#[inline]
pub(crate) fn is_special_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{00A0}'  // non-breaking space
        | '\u{180E}' // mongolian vowel separator
        | '\u{2000}' // en quad
        | '\u{2001}' // em quad
        | '\u{2002}' // en space
        | '\u{2003}' // em space
        | '\u{2004}' // three-per-em space
        | '\u{2005}' // four-per-em space
        | '\u{2006}' // six-per-em space
        | '\u{2007}' // figure space
        | '\u{2008}' // punctuation space
        | '\u{2009}' // thin space
        | '\u{200A}' // hair space
        | '\u{200B}' // zero-width space
        | '\u{202F}' // narrow no-break space
        | '\u{205F}' // medium mathematical space
        | '\u{3000}' // ideographic space
        | '\u{FEFF}' // BOM / zero-width no-break space
    )
}

/// Check if a character is any whitespace (ASCII or special).
#[inline]
pub(crate) fn is_whitespace(c: char) -> bool {
    c.is_ascii_whitespace() || is_special_whitespace(c)
}

/// Check if a character is a valid start of an unquoted string
/// that looks like an identifier (letter or underscore).
#[inline]
pub(crate) fn is_identifier_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

/// Check if a character is valid inside an identifier.
#[inline]
pub(crate) fn is_identifier_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Strip a BOM (byte-order mark) prefix if present.
pub(crate) fn strip_bom(input: &str) -> &str {
    input.strip_prefix('\u{FEFF}').unwrap_or(input)
}
