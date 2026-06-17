use vte::{Params, Perform};

/// Find the closest UTF-8 character boundary at or before the given byte index.
/// This is a stable implementation of String::floor_char_boundary.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    let mut pos = index.min(s.len());
    // Walk backwards until we find a valid UTF-8 start byte
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    pos
}

/// ANSI escape sequence cleaner that extracts plain text from terminal output.
///
/// This handles common control sequences including:
/// - `\r` (carriage return) - clears current line and moves cursor to start
/// - `\n` (line feed) - moves to next line
/// - `\t` (tab) - converts to spaces
/// - `\x08` (backspace) - removes last character
/// - CSI sequences like `\033[K` (clear line), `\033[2J` (clear screen)
///
/// Color codes, cursor movements, and other non-content sequences are ignored.
///
/// Empty lines are classified as either "real" (created by explicit `\n`) or
/// "phantom" (intermediate rows filled when `ESC[row;colH` jumps over them).
/// Phantom empty lines are omitted from output to avoid blank space artifacts
/// from screen-mode rendering sequences.
pub struct AnsiCleaner {
    lines: Vec<String>,
    /// Parallel to `lines`: true = row was explicitly written via `\n`;
    /// false = phantom row filled by a cursor-position jump (H sequence).
    line_is_real: Vec<bool>,
    current_line: String,
    cursor_col: usize, // Track cursor column position for handling cursor movement sequences
    line_cleared: bool, // Track if line was just cleared with \x1b[K
    cursor_row: usize, // Track current row number (0-based)
}

impl AnsiCleaner {
    /// Create a new ANSI cleaner.
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            line_is_real: Vec::new(),
            current_line: String::new(),
            cursor_col: 0,
            line_cleared: false,
            cursor_row: 0,
        }
    }

    /// Process input string and return cleaned plain text.
    pub fn process(&mut self, input: &str) -> String {
        let mut parser = vte::Parser::new();
        parser.advance(self, input.as_bytes());
        // Save last line if it has content
        if !self.current_line.is_empty() {
            self.save_current_line(true);
        }
        self.build_output()
    }

    /// Process input bytes and return cleaned plain text.
    pub fn process_bytes(&mut self, input: &[u8]) -> String {
        let mut parser = vte::Parser::new();
        parser.advance(self, input);
        // Save last line if it has content
        if !self.current_line.is_empty() {
            self.save_current_line(true);
        }
        self.build_output()
    }

    /// Build the final output string, skipping phantom empty lines.
    ///
    /// A "phantom" empty line is one that was never explicitly written by `\n`
    /// but was created as a placeholder when a cursor-position (`H`) sequence
    /// jumped forward over multiple rows.  Real blank lines (from `\n\n`) are
    /// preserved; only phantom ones are dropped.
    fn build_output(&self) -> String {
        let last_non_empty = self.lines.iter().rposition(|l| !l.is_empty());
        let Some(idx) = last_non_empty else {
            return String::new();
        };

        let mut result: Vec<&str> = Vec::with_capacity(idx + 1);
        for (i, line) in self.lines[..=idx].iter().enumerate() {
            let is_real = self.line_is_real.get(i).copied().unwrap_or(false);
            // Keep the line if it has content OR if it was explicitly created by \n.
            // Drop phantom empty lines (H-jump fillers that were never written to).
            if !line.is_empty() || is_real {
                result.push(line.as_str());
            }
        }
        result.join("\n")
    }

    fn ensure_row_exists(&mut self, row: usize) {
        while self.lines.len() <= row {
            self.lines.push(String::new());
            self.line_is_real.push(false);
        }
    }

    fn save_current_line(&mut self, is_real: bool) {
        self.ensure_row_exists(self.cursor_row);
        self.lines[self.cursor_row] = std::mem::take(&mut self.current_line);
        if is_real {
            self.line_is_real[self.cursor_row] = true;
        }
    }

    /// Reset the cleaner state for reuse.
    pub fn reset(&mut self) {
        self.lines.clear();
        self.line_is_real.clear();
        self.current_line.clear();
        self.cursor_col = 0;
        self.line_cleared = false;
        self.cursor_row = 0;
    }
}

impl Default for AnsiCleaner {
    fn default() -> Self {
        Self::new()
    }
}

impl Perform for AnsiCleaner {
    /// Printable characters are added to the current line.
    fn print(&mut self, c: char) {
        // If line was just cleared with \x1b[K and we're adding content at start,
        // clear the entire line (common pattern for progress indicators)
        if self.line_cleared && self.cursor_col == 0 && !self.current_line.is_empty() {
            self.current_line.clear();
        }
        self.line_cleared = false;

        // Handle cursor position:
        // - If cursor is within line, truncate from cursor position and append
        // - If cursor is beyond end, pad with spaces to reach cursor position
        if self.cursor_col < self.current_line.len() {
            // Use floor_char_boundary to ensure we truncate at a valid UTF-8 boundary
            // This prevents panic when cursor_col points to the middle of a multi-byte char (e.g., Chinese)
            let safe_pos = floor_char_boundary(&self.current_line, self.cursor_col);
            self.current_line.truncate(safe_pos);
        } else if self.cursor_col > self.current_line.len() {
            // Pad with spaces to reach cursor position
            let padding = self.cursor_col - self.current_line.len();
            self.current_line.push_str(&" ".repeat(padding));
        }
        self.current_line.push(c);
        self.cursor_col = self.current_line.len();
    }

    /// Execute C0/C1 control characters.
    fn execute(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                // Line feed: move to next line.
                // Intermediate rows pushed here are phantom (cursor jumped over them
                // via a prior B/H sequence without writing).
                // Save current line content at cursor_row position (overwrites if exists).
                // Mark as real: \n explicitly visited this row.
                self.save_current_line(true);
                self.cursor_col = 0;
                self.cursor_row += 1;
                self.line_cleared = false;
            }
            b'\r' => {
                // Carriage return: move cursor to start of line (prepare for overwrite)
                self.cursor_col = 0;
                // If line has content, mark it for potential clearing
                if !self.current_line.is_empty() {
                    self.line_cleared = true;
                }
            }
            b'\t' => {
                // Tab: convert to spaces (8-char tab stop)
                self.line_cleared = false;
                let spaces = 8 - (self.cursor_col % 8);
                self.current_line.push_str(&" ".repeat(spaces));
                self.cursor_col = self.current_line.len();
            }
            b'\x08' => {
                // Backspace: remove last character
                self.line_cleared = false;
                if self.current_line.pop().is_some() && self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            // Other control characters are ignored
            _ => {}
        }
    }

    /// CSI (Control Sequence Introducer) sequences - handle clear operations.
    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, c: char) {
        // Get the first parameter group, default to 1 if empty
        let param = params
            .iter()
            .next()
            .and_then(|group| group.iter().next())
            .unwrap_or(&1);

        match c {
            // Cursor positioning sequences - only update cursor_col, don't modify content
            'G' => {
                // \033[<n>G - Cursor Horizontal Absolute (move to column n, 1-based)
                let target_col = param.saturating_sub(1) as usize;
                self.cursor_col = target_col;
            }
            'C' => {
                // \033[<n>C - Cursor Forward (move forward by n columns)
                let move_cols = *param as usize;
                self.cursor_col = self.cursor_col.saturating_add(move_cols);
            }
            'A' => {
                // \033[<n>A - Cursor Up
                // Move cursor up, can adjust cursor_row if needed
                let move_rows = *param as usize;
                self.cursor_row = self.cursor_row.saturating_sub(move_rows);
            }
            'B' | 'e' => {
                // \033[<n>B or \033[<n>e - Cursor Down
                let move_rows = *param as usize;
                self.cursor_row = self.cursor_row.saturating_add(move_rows);
            }
            'D' => {
                // \033[<n>D - Cursor Backward (move backward by n columns)
                let move_cols = (*param as usize).min(self.cursor_col);
                self.cursor_col -= move_cols;
            }
            'H' | 'f' => {
                // \033[<row>;<col>H or \033[<row>;<col>f - Cursor Position
                //
                // For terminal log cleanup we prefer reconstructing linear text over
                // faithfully emulating the viewport. ConPTY often emits absolute screen
                // coordinates here, which can reuse the same screen rows after scrolling.
                // Treat column 1 as "start a fresh logical line"; treat high-column
                // positions immediately after a line break as a wrapped continuation of
                // the previous logical line.
                let col = params
                    .iter()
                    .nth(1)
                    .and_then(|group| group.iter().next())
                    .unwrap_or(&1u16);
                let target_col = col.saturating_sub(1) as usize;

                if target_col == 0 {
                    if !self.current_line.is_empty() {
                        self.save_current_line(true);
                        self.cursor_row += 1;
                    }
                    self.ensure_row_exists(self.cursor_row);
                    self.current_line = self.lines[self.cursor_row].clone();
                    self.cursor_col = 0;
                    return;
                }

                if self.current_line.is_empty() && self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    self.ensure_row_exists(self.cursor_row);
                    self.current_line = self.lines[self.cursor_row].clone();
                    self.cursor_col = self.current_line.len().saturating_sub(1);
                    return;
                }

                self.cursor_col = target_col;
            }
            // Erase sequences
            'X' => {
                // \033[<n>X - Erase Character (erase n characters ahead of cursor)
                // We ignore this as it will be overwritten by subsequent content
            }
            'K' => {
                // \033[K - Erase in Line
                match *param {
                    0 => {
                        // Erase from cursor to end of line
                        // Use floor_char_boundary to handle multi-byte UTF-8 characters safely
                        let safe_pos = floor_char_boundary(&self.current_line, self.cursor_col);
                        self.current_line.truncate(safe_pos);
                        // Mark line as cleared - if content follows at cursor position, clear entire line
                        if self.cursor_col == 0 {
                            self.line_cleared = true;
                        }
                    }
                    1 | 2 => {
                        // Erase from start to cursor, or entire line
                        self.current_line.clear();
                        self.cursor_col = 0;
                        self.line_cleared = true;
                    }
                    _ => {}
                }
            }
            'J' => {
                // \033[J - Erase in Display
                if *param == 2 {
                    // \033[2J - Erase entire display
                    self.lines.clear();
                    self.line_is_real.clear();
                    self.current_line.clear();
                    self.cursor_col = 0;
                    self.cursor_row = 0;
                }
            }
            // All other CSI sequences are ignored (colors, etc.)
            _ => {}
        }
    }

    /// ESC sequences - we ignore most of them.
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}

    /// OSC (Operating System Command) sequences - ignored.
    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    /// Hook sequences - ignored.
    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}

    /// Put sequences - ignored.
    fn put(&mut self, _byte: u8) {}

    /// Unhook sequences - ignored.
    fn unhook(&mut self) {}
}

/// Convenience function to strip ANSI escape sequences from a string.
///
/// This is a one-shot function that creates a cleaner, processes the input,
/// and returns the cleaned text.
///
/// # Example
/// ```rust
/// use tool_runtime::util::ansi_cleaner::strip_ansi;
///
/// let input = "Loading...\r\x1b[KDone!\n";
/// let cleaned = strip_ansi(input);
/// assert_eq!(cleaned, "Done!");
/// ```
pub fn strip_ansi(input: &str) -> String {
    let mut cleaner = AnsiCleaner::new();
    cleaner.process(input)
}

/// Convenience function to strip ANSI escape sequences from bytes.
pub fn strip_ansi_bytes(input: &[u8]) -> String {
    let mut cleaner = AnsiCleaner::new();
    cleaner.process_bytes(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_text() {
        assert_eq!(strip_ansi("Hello, World!"), "Hello, World!");
    }

    #[test]
    fn test_carriage_return_override() {
        // Progress bar style: \r moves cursor to start, new content overwrites
        let input = "Loading...\rDone!";
        assert_eq!(strip_ansi(input), "Done!");
    }

    #[test]
    fn test_carriage_return_with_clear() {
        // \r then \x1b[K clears entire line when at start
        let input = "Loading...\r\x1b[KDone!";
        assert_eq!(strip_ansi(input), "Done!");
    }

    #[test]
    fn test_clear_line_at_cursor() {
        // \x1b[K with cursor at position 10 only clears from position 10 onwards
        let input = "Loading...\x1b[K";
        // Cursor is at end, so nothing to clear
        assert_eq!(strip_ansi(input), "Loading...");
    }

    #[test]
    fn test_clear_screen() {
        // \x1b[2J clears entire screen
        let input = "Old content\n\x1b[2JNew content";
        assert_eq!(strip_ansi(input), "New content");
    }

    #[test]
    fn test_newlines() {
        let input = "Line 1\nLine 2\nLine 3";
        assert_eq!(strip_ansi(input), "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_backspace() {
        // Backspace removes characters
        let input = "Hello\x08\x08\x08Hi";
        assert_eq!(strip_ansi(input), "HeHi");
    }

    #[test]
    fn test_tab_conversion() {
        // Tabs should be converted to spaces
        let input = "Hello\tWorld";
        let result = strip_ansi(input);
        // "Hello" is 5 chars, so tab adds 3 spaces to reach next 8-char boundary
        assert_eq!(result, "Hello   World");
    }

    #[test]
    fn test_color_codes() {
        // Color codes should be stripped
        let input = "\x1b[31mRed text\x1b[0m normal text";
        assert_eq!(strip_ansi(input), "Red text normal text");
    }

    #[test]
    fn test_complex_terminal_output() {
        // Simulate a realistic progress bar with color and cursor movement
        let input = "\x1b[32mProgress: \x1b[0m10%\r\x1b[K\x1b[32mProgress: \x1b[0m50%\r\x1b[K\x1b[32mProgress: \x1b[0m100%\nDone!";
        assert_eq!(strip_ansi(input), "Progress: 100%\nDone!");
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(strip_ansi(""), "");
    }

    #[test]
    fn test_only_control_sequences() {
        assert_eq!(strip_ansi("\x1b[31m\x1b[0m\r\n"), "");
    }

    #[test]
    fn test_cursor_forward() {
        // \x1b[16C moves cursor forward 16 columns (from current position)
        let input = "d----           2026/1/10\x1b[16C.bitfun";
        // "d----           2026/1/10" is 22 chars, cursor moves from col 22 to col 22+16=38
        // Since 38 > 22, spaces are padded to reach col 38, then ".bitfun" is appended
        assert_eq!(
            strip_ansi(input),
            "d----           2026/1/10                .bitfun"
        );
    }

    #[test]
    fn test_cursor_forward_overwrite() {
        // \x1b[2C moves cursor forward 2 columns to overwrite content
        let input = "Hello World\x1b[2C!!";
        // "Hello World" is 11 chars, cursor moves to col 11+2=13, spaces are padded, then "!!" appended
        assert_eq!(strip_ansi(input), "Hello World  !!");
    }

    #[test]
    fn test_cursor_horizontal_absolute() {
        // \x1b[16G moves cursor to column 16 (1-based)
        let input = "Mode  LastWriteTime\x1b[16GLength";
        // "Mode  LastWriteTime" is 18 chars, cursor moves to col 16 (0-based: 15)
        // Content from col 16 onwards is replaced
        assert_eq!(strip_ansi(input), "Mode  LastWriteLength");
    }

    #[test]
    fn test_cursor_position() {
        // \x1b[5;1H is treated as moving to the next logical line.
        let input = "Header\x1b[5;1HNew content";
        assert_eq!(strip_ansi(input), "Header\nNew content");
    }

    #[test]
    fn test_cursor_backward() {
        // \x1b[3D moves cursor backward 3 columns
        let input = "Hello World\x1b[3D!";
        // "Hello World" is 11 chars, cursor moves back from col 11 to col 11-3=8
        // "!" overwrites from position 8
        assert_eq!(strip_ansi(input), "Hello Wo!");
    }

    #[test]
    fn test_tab_after_cursor_movement() {
        // \x1b[10G moves cursor to column 10 (1-based, so 9 in 0-based)
        // Tab should then move to next 8-char boundary from cursor position
        let input = "Hello\x1b[10G\tWorld";
        // "Hello" is 5 chars, cursor moves to column 9 (0-based)
        // Tab adds 8 - (9 % 8) = 7 spaces to reach column 16 (next 8-char boundary)
        // Then "World" is appended
        assert_eq!(strip_ansi(input), "Hello       World");
    }

    #[test]
    fn human_written_test() {
        let input = "\u{001b}[93mls\u{001b}[K\r\n\u{001b}[?25h\u{001b}[m\r\n\u{001b}[?25l    Directory: E:\\Projects\\ForTest\\basic-rust\u{001b}[32m\u{001b}[1m\u{001b}[5;1HMode                 LastWriteTime\u{001b}[m \u{001b}[32m\u{001b}[1m\u{001b}[3m        Length\u{001b}[23m Name\r\n----   \u{001b}[m \u{001b}[32m\u{001b}[1m             -------------\u{001b}[m \u{001b}[32m\u{001b}[1m        ------\u{001b}[m \u{001b}[32m\u{001b}[1m----\u{001b}[m\r\nd----           2026/1/10    19:23\u{001b}[16X\u{001b}[44m\u{001b}[1m\u{001b}[16C.bitfun\u{001b}[m\r\nd----           2026/1/10    21:18\u{001b}[16X\u{001b}[44m\u{001b}[1m\u{001b}[16C.worktrees\u{001b}[m\r\nd----           2026/1/10    19:21\u{001b}[16X\u{001b}[44m\u{001b}[1m\u{001b}[16Csrc\u{001b}[m\r\nd----           2026/1/10    19:21\u{001b}[16X\u{001b}[44m\u{001b}[1m\u{001b}[16Ctarget\r\n\u{001b}[?25h\u{001b}[?25l\u{001b}[m-a---           2026/1/10    19:23             57 .gitignore\r\n-a---           2026/1/10    19:21            154 Cargo.lock\r\n-a---           2026/1/10    19:21             81 Cargo.toml\u{001b}[15;1H\u{001b}[?25h";
        // The blank line between "Directory:" and "Mode..." was produced by
        // ESC[5;1H jumping from row 2 to row 4, leaving row 3 as a phantom
        // empty line.  With phantom-line filtering it is now omitted.
        let expected_output = "ls\n\n    Directory: E:\\Projects\\ForTest\\basic-rust\nMode                 LastWriteTime         Length Name\n----                 -------------         ------ ----\nd----           2026/1/10    19:23                .bitfun\nd----           2026/1/10    21:18                .worktrees\nd----           2026/1/10    19:21                src\nd----           2026/1/10    19:21                target\n-a---           2026/1/10    19:23             57 .gitignore\n-a---           2026/1/10    19:21            154 Cargo.lock\n-a---           2026/1/10    19:21             81 Cargo.toml";
        assert_eq!(strip_ansi(input), expected_output);
    }

    #[test]
    fn test_cursor_position_soft_wrap_continuation() {
        let input =
            "This will stop working in the next maj\r\n\x1b[49;83Hjor version of npm.\r\nNext line";
        assert_eq!(
            strip_ansi(input),
            "This will stop working in the next major version of npm.\nNext line"
        );
    }

    #[test]
    fn test_cursor_position_reused_screen_rows_do_not_overwrite_previous_lines() {
        let input = concat!(
            "First warning line ends in maj\r\n",
            "\x1b[49;83Hjor version one.\r\n",
            "Second warning line ends in maj\r\n",
            "\x1b[49;83Hjor version two.\r\n",
            "Final line"
        );
        assert_eq!(
            strip_ansi(input),
            concat!(
                "First warning line ends in major version one.\n",
                "Second warning line ends in major version two.\n",
                "Final line"
            )
        );
    }

    #[test]
    fn test_multibyte_chinese_basic() {
        // Basic Chinese text should pass through correctly
        let input = "你好世界！Hello World";
        assert_eq!(strip_ansi(input), "你好世界！Hello World");
    }

    #[test]
    fn test_multibyte_chinese_carriage_return() {
        // Chinese text with carriage return override
        let input = "加载中...\r完成！";
        assert_eq!(strip_ansi(input), "完成！");
    }

    #[test]
    fn test_multibyte_chinese_backspace() {
        // Backspace with multi-byte characters
        // Note: cursor_col tracks bytes, so backspace behavior with multi-byte chars
        // may add padding spaces. This is a known limitation.
        // "你好世界" (12 bytes) → backspace removes "界" (3 bytes) → "你好世" (9 bytes)
        // cursor_col goes from 12 to 11 (decremented by 1)
        // When adding '！', padding = 11 - 9 = 2 spaces
        let input = "你好世界\x08！";
        assert_eq!(strip_ansi(input), "你好世  ！");

        // Simple backspace with ASCII works correctly
        let input2 = "Hello\x08\x08Hi";
        assert_eq!(strip_ansi(input2), "HelHi");
    }

    #[test]
    fn test_multibyte_chinese_cursor_truncate() {
        // Cursor movement with truncate - this previously caused panic
        // "你好世界" (12 bytes for 4 Chinese chars)
        // \x1b[5G moves cursor to column 5 (0-based: 4)
        // This would truncate in the middle of the second Chinese character
        // After fix, it should floor to the nearest character boundary
        let input = "你好世界\x1b[5G测试";
        // cursor moves to column 4, which is in the middle of "好"
        // floor_char_boundary should adjust to column 3 (end of "你")
        // Then truncate and append "测试"
        let result = strip_ansi(input);
        // Should not panic and handle gracefully
        assert!(!result.is_empty());
    }

    #[test]
    fn test_multibyte_chinese_clear_line() {
        // Chinese text with clear line sequence
        let input = "处理中...\x1b[K完成！";
        assert_eq!(strip_ansi(input), "处理中...完成！");
    }

    #[test]
    fn test_multibyte_mixed_width_chars() {
        // Mix of ASCII and Chinese with cursor movement
        let input = "Progress: 进度中\r\x1b[KDone!";
        assert_eq!(strip_ansi(input), "Done!");
    }

    #[test]
    fn test_multibyte_emoji() {
        // Emoji (4-byte UTF-8) should also work
        let input = "Loading 😊😊\r\x1b[KDone ✓";
        assert_eq!(strip_ansi(input), "Done ✓");
    }

    #[test]
    fn test_multibyte_japanese() {
        // Japanese characters
        let input = "読み込み中...\r完了！";
        assert_eq!(strip_ansi(input), "完了！");
    }

    #[test]
    fn test_multibyte_korean() {
        // Korean characters
        let input = "로딩 중...\r완료!";
        assert_eq!(strip_ansi(input), "완료!");
    }

    #[test]
    fn test_multibyte_arabic() {
        // Arabic characters (RTL script, but stored LTR in UTF-8)
        let input = "جاري التحميل...\rتم!";
        assert_eq!(strip_ansi(input), "تم!");
    }
}

// A command to generate test data
// Write-Host "=== ANSI Test ===" -ForegroundColor Cyan; Write-Host ""; for ($i=0;$i -le 100;$i+=5){$progress='='*($i/5);$empty=' '*(20-($i/5));Write-Host -NoNewline "`r[$progress$empty] $i% ";Start-Sleep -Milliseconds 100}; Write-Host ""; Write-Host ""; $esc=[char]27; Write-Host "${esc}[31mRed${esc}[0m ${esc}[32mGreen${esc}[0m ${esc}[33mYellow${esc}[0m ${esc}[34mBlue${esc}[0m ${esc}[35mMagenta${esc}[0m ${esc}[36mCyan${esc}[0m"; Write-Host "${esc}[1m${esc}[31mBold Red${esc}[0m ${esc}[4m${esc}[32mUnderline Green${esc}[0m ${esc}[5m${esc}[33mBlink Yellow${esc}[0m ${esc}[7m${esc}[34mReverse Blue${esc}[0m"; Write-Host ""; Write-Host "=== Process List ===" -ForegroundColor Yellow; Get-Process | Select-Object -First 5 Name,Id,CPU | Format-Table -AutoSize
