/// Theme and style definitions

use ratatui::style::{Color, Modifier, Style};
use std::io::{IsTerminal, Read};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

#[derive(Debug, Clone)]
pub struct Theme {
    pub primary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub muted: Color,
    pub background: Color,
    pub border: Color,

    // Panel backgrounds (inspired by opencode theme)
    pub background_panel: Color,
    pub background_element: Color,

    // Diff colors
    pub diff_added_fg: Color,
    pub diff_removed_fg: Color,
    pub diff_added_bg: Color,
    pub diff_removed_bg: Color,

    // Block card colors
    pub block_bg: Color,
    pub block_bg_hover: Color,
    pub block_border_active: Color,

    // Inline tool icon color
    pub inline_icon: Color,

    // Command text color (for bash $ prefix)
    pub command_text: Color,

    // Diff hunk header and line number colors
    pub diff_hunk_header: Color,
    pub diff_line_number: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveColorScheme {
    Truecolor,
    Ansi16,
    Monochrome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Dark,
    Light,
}

impl Appearance {
    pub fn is_light(self) -> bool {
        matches!(self, Appearance::Light)
    }
}

pub fn resolve_effective_color_scheme(preference: &str) -> EffectiveColorScheme {
    if std::env::var_os("NO_COLOR").is_some() {
        return EffectiveColorScheme::Monochrome;
    }
    if matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0")) {
        return EffectiveColorScheme::Monochrome;
    }

    match preference.trim().to_ascii_lowercase().as_str() {
        "mono" | "monochrome" | "nocolor" | "no_color" => EffectiveColorScheme::Monochrome,
        "ansi" | "ansi16" => EffectiveColorScheme::Ansi16,
        "truecolor" | "24bit" => EffectiveColorScheme::Truecolor,
        "" | "default" | "auto" | _ => {
            if terminal_supports_truecolor() {
                EffectiveColorScheme::Truecolor
            } else {
                EffectiveColorScheme::Ansi16
            }
        }
    }
}

fn terminal_supports_truecolor() -> bool {
    if !std::io::stdout().is_terminal() {
        return false;
    }

    let term = std::env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    if term.is_empty() || term == "dumb" {
        return false;
    }

    let term_program = std::env::var("TERM_PROGRAM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if term_program == "apple_terminal" {
        return false;
    }

    let colorterm = std::env::var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return true;
    }

    // Many terminals expose truecolor capability via TERM_PROGRAM.
    // Apple Terminal historically lacks reliable 24-bit support across versions/configurations,
    // so we keep detection conservative and prefer ANSI16 in ambiguous cases.
    if term_program == "iterm.app"
        || term_program == "wezterm"
        || term_program == "vscode"
        || term_program == "ghostty"
    {
        return true;
    }

    // Some terminals encode truecolor as "-direct" terminfo.
    if term.contains("-direct") || term.contains("xterm-kitty") {
        return true;
    }

    false
}

pub fn resolve_appearance(preference: &str) -> Appearance {
    match preference.trim().to_ascii_lowercase().as_str() {
        "light" => Appearance::Light,
        "auto" => detect_terminal_appearance(Duration::from_millis(250)).unwrap_or(Appearance::Dark),
        _ => Appearance::Dark,
    }
}

fn detect_terminal_appearance(timeout: Duration) -> Option<Appearance> {
    if std::env::var_os("NO_COLOR").is_some() {
        return None;
    }
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        return None;
    }

    // OSC 11 query: request default background color.
    // Response typically looks like:
    //   ESC ] 11 ; rgb:RRRR/GGGG/BBBB BEL
    // or:
    //   ESC ] 11 ; #RRGGBB BEL
    use std::io::Write;

    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(b"\x1b]11;?\x07");
    let _ = stdout.flush();

    let start = Instant::now();

    // Read from stdin in non-blocking mode (Unix-only best-effort).
    #[cfg(unix)]
    {
        let mut buf = Vec::with_capacity(256);
        use std::os::fd::AsRawFd;

        let fd = std::io::stdin().as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            if flags < 0 {
                return None;
            }
            if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                return None;
            }

            let mut stdin = std::io::stdin().lock();
            let mut tmp = [0u8; 256];
            while start.elapsed() < timeout {
                match stdin.read(&mut tmp) {
                    Ok(0) => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if buf.contains(&b'\x07') {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }

            let _ = libc::fcntl(fd, libc::F_SETFL, flags);
        }

        let s = String::from_utf8_lossy(&buf);
        let prefix = "\u{1b}]11;";
        let idx = s.find(prefix)?;
        let rest = &s[idx + prefix.len()..];
        let end = rest.find('\u{7}').unwrap_or(rest.len());
        let color = rest[..end].trim();

        let (r, g, b) = parse_osc_color(color)?;
        let lum = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0;
        Some(if lum > 0.5 { Appearance::Light } else { Appearance::Dark })
    }

    #[cfg(not(unix))]
    {
        let _ = (start, timeout);
        None
    }
}

#[allow(dead_code)]
fn parse_osc_color(s: &str) -> Option<(u8, u8, u8)> {
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some((r, g, b));
        }
    }

    if let Some(rgb) = s.strip_prefix("rgb:") {
        let parts: Vec<&str> = rgb.split('/').collect();
        if parts.len() == 3 {
            let r16 = u16::from_str_radix(parts[0], 16).ok()?;
            let g16 = u16::from_str_radix(parts[1], 16).ok()?;
            let b16 = u16::from_str_radix(parts[2], 16).ok()?;
            return Some(((r16 >> 8) as u8, (g16 >> 8) as u8, (b16 >> 8) as u8));
        }
    }

    if let Some(rgb) = s.strip_prefix("rgb(").and_then(|t| t.strip_suffix(')')) {
        let parts: Vec<&str> = rgb.split(',').map(|p| p.trim()).collect();
        if parts.len() == 3 {
            let r = parts[0].parse::<u8>().ok()?;
            let g = parts[1].parse::<u8>().ok()?;
            let b = parts[2].parse::<u8>().ok()?;
            return Some((r, g, b));
        }
    }

    None
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            primary: Color::Rgb(59, 130, 246),    // blue
            success: Color::Rgb(34, 197, 94),     // green
            warning: Color::Rgb(251, 191, 36),    // yellow
            error: Color::Rgb(239, 68, 68),       // red
            info: Color::Rgb(147, 197, 253),      // light blue
            muted: Color::Rgb(156, 163, 175),     // gray
            background: Color::Rgb(17, 24, 39),   // dark gray background
            border: Color::Rgb(55, 65, 81),       // border gray

            background_panel: Color::Rgb(30, 38, 55),
            background_element: Color::Rgb(40, 50, 70),

            diff_added_fg: Color::Rgb(34, 197, 94),
            diff_removed_fg: Color::Rgb(239, 68, 68),
            diff_added_bg: Color::Rgb(20, 50, 20),
            diff_removed_bg: Color::Rgb(50, 20, 20),

            block_bg: Color::Rgb(24, 32, 48),
            block_bg_hover: Color::Rgb(32, 42, 62),
            block_border_active: Color::Rgb(59, 130, 246),

            inline_icon: Color::Rgb(100, 140, 220),

            command_text: Color::Rgb(180, 210, 255),

            diff_hunk_header: Color::Rgb(100, 120, 180),
            diff_line_number: Color::Rgb(80, 90, 110),
        }
    }

    pub fn dark_ansi16() -> Self {
        Self {
            primary: Color::Blue,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Cyan,
            muted: Color::DarkGray,
            background: Color::Reset,
            border: Color::DarkGray,

            background_panel: Color::Reset,
            background_element: Color::Reset,

            diff_added_fg: Color::Green,
            diff_removed_fg: Color::Red,
            diff_added_bg: Color::Reset,
            diff_removed_bg: Color::Reset,

            block_bg: Color::Reset,
            block_bg_hover: Color::Reset,
            block_border_active: Color::Blue,

            inline_icon: Color::Blue,

            command_text: Color::Cyan,

            diff_hunk_header: Color::Magenta,
            diff_line_number: Color::DarkGray,
        }
    }

    pub fn light() -> Self {
        Self {
            primary: Color::Rgb(37, 99, 235),
            success: Color::Rgb(22, 163, 74),
            warning: Color::Rgb(245, 158, 11),
            error: Color::Rgb(220, 38, 38),
            info: Color::Rgb(59, 130, 246),
            muted: Color::Rgb(107, 114, 128),
            background: Color::Rgb(249, 250, 251),
            border: Color::Rgb(209, 213, 219),

            background_panel: Color::Rgb(243, 244, 246),
            background_element: Color::Rgb(229, 231, 235),

            diff_added_fg: Color::Rgb(22, 163, 74),
            diff_removed_fg: Color::Rgb(220, 38, 38),
            diff_added_bg: Color::Rgb(220, 252, 231),
            diff_removed_bg: Color::Rgb(254, 226, 226),

            block_bg: Color::Rgb(240, 242, 245),
            block_bg_hover: Color::Rgb(232, 235, 240),
            block_border_active: Color::Rgb(37, 99, 235),

            inline_icon: Color::Rgb(60, 100, 200),

            command_text: Color::Rgb(30, 60, 120),

            diff_hunk_header: Color::Rgb(80, 100, 160),
            diff_line_number: Color::Rgb(140, 150, 170),
        }
    }

    pub fn light_ansi16() -> Self {
        Self {
            primary: Color::Blue,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Cyan,
            muted: Color::DarkGray,
            background: Color::Reset,
            border: Color::DarkGray,

            background_panel: Color::Reset,
            background_element: Color::Reset,

            diff_added_fg: Color::Green,
            diff_removed_fg: Color::Red,
            diff_added_bg: Color::Reset,
            diff_removed_bg: Color::Reset,

            block_bg: Color::Reset,
            block_bg_hover: Color::Reset,
            block_border_active: Color::Blue,

            inline_icon: Color::Blue,

            command_text: Color::Blue,

            diff_hunk_header: Color::Magenta,
            diff_line_number: Color::DarkGray,
        }
    }

    pub fn monochrome() -> Self {
        Self {
            primary: Color::Reset,
            success: Color::Reset,
            warning: Color::Reset,
            error: Color::Reset,
            info: Color::Reset,
            muted: Color::Reset,
            background: Color::Reset,
            border: Color::Reset,

            background_panel: Color::Reset,
            background_element: Color::Reset,

            diff_added_fg: Color::Reset,
            diff_removed_fg: Color::Reset,
            diff_added_bg: Color::Reset,
            diff_removed_bg: Color::Reset,

            block_bg: Color::Reset,
            block_bg_hover: Color::Reset,
            block_border_active: Color::Reset,

            inline_icon: Color::Reset,

            command_text: Color::Reset,

            diff_hunk_header: Color::Reset,
            diff_line_number: Color::Reset,
        }
    }

    pub fn with_effective_scheme(mut self, scheme: EffectiveColorScheme) -> Self {
        match scheme {
            EffectiveColorScheme::Truecolor => self,
            EffectiveColorScheme::Monochrome => Theme::monochrome(),
            EffectiveColorScheme::Ansi16 => {
                self.primary = to_ansi16(self.primary);
                self.success = to_ansi16(self.success);
                self.warning = to_ansi16(self.warning);
                self.error = to_ansi16(self.error);
                self.info = to_ansi16(self.info);
                self.muted = to_ansi16(self.muted);
                self.background = to_ansi16(self.background);
                self.border = to_ansi16(self.border);
                self.background_panel = to_ansi16(self.background_panel);
                self.background_element = to_ansi16(self.background_element);
                self.diff_added_fg = to_ansi16(self.diff_added_fg);
                self.diff_removed_fg = to_ansi16(self.diff_removed_fg);
                self.diff_added_bg = to_ansi16(self.diff_added_bg);
                self.diff_removed_bg = to_ansi16(self.diff_removed_bg);
                self.block_bg = to_ansi16(self.block_bg);
                self.block_bg_hover = to_ansi16(self.block_bg_hover);
                self.block_border_active = to_ansi16(self.block_border_active);
                self.inline_icon = to_ansi16(self.inline_icon);
                self.command_text = to_ansi16(self.command_text);
                self.diff_hunk_header = to_ansi16(self.diff_hunk_header);
                self.diff_line_number = to_ansi16(self.diff_line_number);
                self
            }
        }
    }

    pub fn apply_opencode_theme_json(
        &self,
        json: &OpencodeThemeJson,
        appearance: Appearance,
    ) -> anyhow::Result<Self> {
        let fallback = self.clone();
        let resolved = resolve_opencode_theme(json, appearance)?;
        Ok(Theme {
            primary: resolved.primary.unwrap_or(fallback.primary),
            success: resolved.success.unwrap_or(fallback.success),
            warning: resolved.warning.unwrap_or(fallback.warning),
            error: resolved.error.unwrap_or(fallback.error),
            info: resolved.info.unwrap_or(fallback.info),
            muted: resolved.muted.unwrap_or(fallback.muted),
            background: resolved.background.unwrap_or(fallback.background),
            border: resolved.border.unwrap_or(fallback.border),
            background_panel: resolved.background_panel.unwrap_or(fallback.background_panel),
            background_element: resolved.background_element.unwrap_or(fallback.background_element),
            diff_added_fg: resolved.diff_added_fg.unwrap_or(fallback.diff_added_fg),
            diff_removed_fg: resolved.diff_removed_fg.unwrap_or(fallback.diff_removed_fg),
            diff_added_bg: resolved.diff_added_bg.unwrap_or(fallback.diff_added_bg),
            diff_removed_bg: resolved.diff_removed_bg.unwrap_or(fallback.diff_removed_bg),
            block_bg: resolved.block_bg.unwrap_or(fallback.block_bg),
            block_bg_hover: resolved.block_bg_hover.unwrap_or(fallback.block_bg_hover),
            block_border_active: resolved.block_border_active.unwrap_or(fallback.block_border_active),
            inline_icon: resolved.inline_icon.unwrap_or(fallback.inline_icon),
            command_text: resolved.command_text.unwrap_or(fallback.command_text),
            diff_hunk_header: resolved.diff_hunk_header.unwrap_or(fallback.diff_hunk_header),
            diff_line_number: resolved.diff_line_number.unwrap_or(fallback.diff_line_number),
        })
    }

    pub fn style(&self, kind: StyleKind) -> Style {
        match kind {
            StyleKind::Primary => Style::default().fg(self.primary),
            StyleKind::Success => Style::default().fg(self.success),
            StyleKind::Warning => Style::default().fg(self.warning),
            StyleKind::Error => Style::default().fg(self.error),
            StyleKind::Info => Style::default().fg(self.info),
            StyleKind::Muted => Style::default().fg(self.muted),
            StyleKind::Title => Style::default()
                .fg(self.primary)
                .add_modifier(Modifier::BOLD),
            StyleKind::Border => Style::default().fg(self.border),
            StyleKind::DiffAdded => Style::default().fg(self.diff_added_fg).bg(self.diff_added_bg),
            StyleKind::DiffRemoved => Style::default().fg(self.diff_removed_fg).bg(self.diff_removed_bg),
            StyleKind::BackgroundPanel => Style::default().bg(self.background_panel),
            StyleKind::BackgroundElement => Style::default().bg(self.background_element),
            StyleKind::BlockBackground => Style::default().bg(self.block_bg),
            StyleKind::BlockBackgroundHover => Style::default().bg(self.block_bg_hover),
            StyleKind::BlockBorderActive => Style::default().fg(self.block_border_active),
            StyleKind::InlineIcon => Style::default().fg(self.inline_icon),
            StyleKind::CommandText => Style::default().fg(self.command_text),
            StyleKind::DiffHunkHeader => Style::default().fg(self.diff_hunk_header),
            StyleKind::DiffLineNumber => Style::default().fg(self.diff_line_number),
        }
    }
}

fn to_ansi16(c: Color) -> Color {
    match c {
        Color::Reset => Color::Reset,
        Color::Black
        | Color::Red
        | Color::Green
        | Color::Yellow
        | Color::Blue
        | Color::Magenta
        | Color::Cyan
        | Color::Gray
        | Color::DarkGray
        | Color::LightRed
        | Color::LightGreen
        | Color::LightYellow
        | Color::LightBlue
        | Color::LightMagenta
        | Color::LightCyan
        | Color::White => c,
        Color::Indexed(idx) => idx_to_ansi16(idx),
        Color::Rgb(r, g, b) => rgb_to_ansi16(r, g, b),
    }
}

fn idx_to_ansi16(idx: u8) -> Color {
    // Basic mapping for 0-15.
    match idx {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::Gray,
        8 => Color::DarkGray,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::White,
        _ => Color::Gray,
    }
}

fn rgb_to_ansi16(r: u8, g: u8, b: u8) -> Color {
    // Simple luminance+dominant-channel approximation.
    let lum = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) / 255.0;
    if lum < 0.08 {
        return Color::Black;
    }
    if lum > 0.92 {
        return Color::White;
    }

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let sat = (max - min) as f64 / 255.0;
    let bright = lum > 0.6;

    if sat < 0.18 {
        return if bright { Color::Gray } else { Color::DarkGray };
    }

    let is_r = r == max;
    let is_g = g == max;
    let is_b = b == max;

    match (is_r, is_g, is_b, bright) {
        (true, true, false, false) => Color::Yellow,
        (true, true, false, true) => Color::LightYellow,
        (true, false, true, false) => Color::Magenta,
        (true, false, true, true) => Color::LightMagenta,
        (false, true, true, false) => Color::Cyan,
        (false, true, true, true) => Color::LightCyan,
        (true, false, false, false) => Color::Red,
        (true, false, false, true) => Color::LightRed,
        (false, true, false, false) => Color::Green,
        (false, true, false, true) => Color::LightGreen,
        (false, false, true, false) => Color::Blue,
        (false, false, true, true) => Color::LightBlue,
        _ => Color::Gray,
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum StyleKind {
    Primary,
    Success,
    Warning,
    Error,
    Info,
    Muted,
    Title,
    Border,
    DiffAdded,
    DiffRemoved,
    BackgroundPanel,
    BackgroundElement,
    BlockBackground,
    BlockBackgroundHover,
    BlockBorderActive,
    InlineIcon,
    CommandText,
    DiffHunkHeader,
    DiffLineNumber,
}

/// Tool icon mapping — Unicode symbols inspired by opencode TUI
pub fn tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        "Bash" | "bash_tool" | "run_terminal_cmd" => "$",
        "Read" | "read_file" | "read_file_tool" => "\u{2192}",  // →
        "Write" | "write_file" | "write_file_tool" => "\u{2190}", // ←
        "Edit" | "search_replace" => "\u{2190}",                  // ←
        "Delete" => "\u{00d7}",                                    // ×
        "Grep" | "grep" => "\u{2731}",                             // ✱
        "Glob" | "codebase_search" => "\u{2731}",                  // ✱
        "LS" | "list_dir" | "ls" => "\u{2192}",                   // →
        "WebFetch" => "%",
        "WebSearch" => "\u{25c8}",                                 // ◈
        "Task" => "#",
        "HmosCompilation" => "\u{2692}",
        "TodoWrite" => "\u{2699}",                                 // ⚙
        "Skill" => "\u{2192}",                                    // →
        "Git" => "\u{2387}",                                       // ⎇
        "AskUserQuestion" => "?",
        "CreatePlan" => "\u{25b6}",                                // ▶
        "ReadLints" => "\u{25b3}",                                 // △
        "GetFileDiff" => "\u{00b1}",                               // ±
        "IdeControl" => "\u{2318}",                                // ⌘
        "MermaidInteractive" => "\u{25c7}",                        // ◇
        "ContextCompression" => "\u{21af}",                        // ↯
        "AnalyzeImage" => "\u{25a3}",                              // ▣
        _ if tool_name.starts_with("mcp_") => "\u{2261}",         // ≡
        _ => "\u{00b7}",                                           // ·
    }
}

// ======================= opencode-compatible theme.json =======================

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OpencodeThemeJson {
    #[serde(rename = "$schema")]
    #[allow(dead_code)]
    pub schema: Option<String>,
    #[allow(dead_code)]
    pub defs: Option<HashMap<String, ColorValueJson>>,
    pub theme: HashMap<String, ColorValueJson>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum ColorValueJson {
    Number(u8),
    String(String),
    Variant {
        dark: Box<ColorValueJson>,
        light: Box<ColorValueJson>,
    },
}

#[allow(dead_code)]
pub fn load_opencode_theme_json(path: &Path) -> anyhow::Result<OpencodeThemeJson> {
    let data = std::fs::read_to_string(path)?;
    let json = serde_json::from_str::<OpencodeThemeJson>(&data)?;
    Ok(json)
}

static BUILTIN_OPENCODE_THEMES: Lazy<HashMap<&'static str, OpencodeThemeJson>> = Lazy::new(|| {
    fn parse(id: &'static str, raw: &'static str) -> ( &'static str, OpencodeThemeJson) {
        let json = serde_json::from_str::<OpencodeThemeJson>(raw)
            .unwrap_or_else(|e| panic!("Failed to parse built-in theme {}: {}", id, e));
        (id, json)
    }

    HashMap::from([
        parse(
            "aura",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/themes/presets/aura.json")),
        ),
        parse(
            "ayu",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/themes/presets/ayu.json")),
        ),
        parse(
            "carbonfox",
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/themes/presets/carbonfox.json")),
        ),
        parse(
            "catppuccin-frappe",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/catppuccin-frappe.json"
            )),
        ),
        parse(
            "catppuccin-macchiato",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/catppuccin-macchiato.json"
            )),
        ),
        parse(
            "catppuccin",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/catppuccin.json"
            )),
        ),
        parse(
            "cobalt2",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/cobalt2.json"
            )),
        ),
        parse(
            "cursor",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/cursor.json"
            )),
        ),
        parse(
            "dracula",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/dracula.json"
            )),
        ),
        parse(
            "everforest",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/everforest.json"
            )),
        ),
        parse(
            "flexoki",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/flexoki.json"
            )),
        ),
        parse(
            "github",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/github.json"
            )),
        ),
        parse(
            "gruvbox",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/gruvbox.json"
            )),
        ),
        parse(
            "kanagawa",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/kanagawa.json"
            )),
        ),
        parse(
            "lucent-orng",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/lucent-orng.json"
            )),
        ),
        parse(
            "material",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/material.json"
            )),
        ),
        parse(
            "matrix",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/matrix.json"
            )),
        ),
        parse(
            "mercury",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/mercury.json"
            )),
        ),
        parse(
            "monokai",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/monokai.json"
            )),
        ),
        parse(
            "nightowl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/nightowl.json"
            )),
        ),
        parse(
            "nord",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/nord.json"
            )),
        ),
        parse(
            "one-dark",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/one-dark.json"
            )),
        ),
        parse(
            "opencode",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/opencode.json"
            )),
        ),
        parse(
            "orng",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/orng.json"
            )),
        ),
        parse(
            "osaka-jade",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/osaka-jade.json"
            )),
        ),
        parse(
            "palenight",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/palenight.json"
            )),
        ),
        parse(
            "rosepine",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/rosepine.json"
            )),
        ),
        parse(
            "solarized",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/solarized.json"
            )),
        ),
        parse(
            "synthwave84",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/synthwave84.json"
            )),
        ),
        parse(
            "tokyonight",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/tokyonight.json"
            )),
        ),
        parse(
            "vercel",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/vercel.json"
            )),
        ),
        parse(
            "vesper",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/vesper.json"
            )),
        ),
        parse(
            "zenburn",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/themes/presets/zenburn.json"
            )),
        ),
    ])
});

pub fn builtin_theme_ids() -> Vec<String> {
    let mut ids: Vec<String> = BUILTIN_OPENCODE_THEMES.keys().map(|k| (*k).to_string()).collect();
    ids.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    ids
}

pub fn builtin_theme_json(id: &str) -> Option<&'static OpencodeThemeJson> {
    BUILTIN_OPENCODE_THEMES.get(id)
}

#[derive(Debug, Default)]
struct ResolvedTokens {
    primary: Option<Color>,
    success: Option<Color>,
    warning: Option<Color>,
    error: Option<Color>,
    info: Option<Color>,
    muted: Option<Color>,
    background: Option<Color>,
    border: Option<Color>,
    background_panel: Option<Color>,
    background_element: Option<Color>,
    diff_added_fg: Option<Color>,
    diff_removed_fg: Option<Color>,
    diff_added_bg: Option<Color>,
    diff_removed_bg: Option<Color>,
    block_bg: Option<Color>,
    block_bg_hover: Option<Color>,
    block_border_active: Option<Color>,
    inline_icon: Option<Color>,
    command_text: Option<Color>,
    diff_hunk_header: Option<Color>,
    diff_line_number: Option<Color>,
}

fn resolve_opencode_theme(json: &OpencodeThemeJson, appearance: Appearance) -> anyhow::Result<ResolvedTokens> {
    let mode = if appearance.is_light() { "light" } else { "dark" };
    let defs = json.defs.clone().unwrap_or_default();

    let mut tokens = ResolvedTokens::default();

    tokens.primary = resolve_key(json, &defs, "primary", mode)?;
    tokens.success = resolve_key(json, &defs, "success", mode)?;
    tokens.warning = resolve_key(json, &defs, "warning", mode)?;
    tokens.error = resolve_key(json, &defs, "error", mode)?;
    tokens.info = resolve_key(json, &defs, "info", mode)?;
    tokens.muted = resolve_key(json, &defs, "textMuted", mode)?;
    tokens.background = resolve_key(json, &defs, "background", mode)?;
    tokens.border = resolve_key(json, &defs, "border", mode)?;
    tokens.background_panel = resolve_key(json, &defs, "backgroundPanel", mode)?;
    tokens.background_element = resolve_key(json, &defs, "backgroundElement", mode)?;

    tokens.diff_added_fg = resolve_key(json, &defs, "diffAdded", mode)?;
    tokens.diff_removed_fg = resolve_key(json, &defs, "diffRemoved", mode)?;
    tokens.diff_added_bg = resolve_key(json, &defs, "diffAddedBg", mode)?;
    tokens.diff_removed_bg = resolve_key(json, &defs, "diffRemovedBg", mode)?;

    tokens.block_bg = tokens.background_panel.clone();
    tokens.block_bg_hover = tokens.background_element.clone();

    tokens.block_border_active =
        resolve_key(json, &defs, "borderActive", mode)?.or(tokens.primary.clone());
    tokens.inline_icon = resolve_key(json, &defs, "accent", mode)?.or(tokens.primary.clone());
    tokens.command_text = resolve_key(json, &defs, "accent", mode)?.or(tokens.info.clone());
    tokens.diff_hunk_header = resolve_key(json, &defs, "diffHunkHeader", mode)?;
    tokens.diff_line_number = resolve_key(json, &defs, "diffLineNumber", mode)?;

    Ok(tokens)
}

fn resolve_key(
    json: &OpencodeThemeJson,
    defs: &HashMap<String, ColorValueJson>,
    key: &str,
    mode: &str,
) -> anyhow::Result<Option<Color>> {
    let Some(v) = json.theme.get(key) else {
        return Ok(None);
    };
    let mut seen = HashSet::<String>::new();
    Ok(Some(resolve_color_value(json, defs, v, mode, &mut seen)?))
}

fn resolve_color_value(
    json: &OpencodeThemeJson,
    defs: &HashMap<String, ColorValueJson>,
    v: &ColorValueJson,
    mode: &str,
    seen: &mut HashSet<String>,
) -> anyhow::Result<Color> {
    match v {
        ColorValueJson::Number(n) => Ok(Color::Indexed(*n)),
        ColorValueJson::Variant { dark, light } => {
            if mode == "light" {
                resolve_color_value(json, defs, light, mode, seen)
            } else {
                resolve_color_value(json, defs, dark, mode, seen)
            }
        }
        ColorValueJson::String(s) => resolve_color_string(json, defs, s, mode, seen),
    }
}

fn resolve_color_string(
    json: &OpencodeThemeJson,
    defs: &HashMap<String, ColorValueJson>,
    s: &str,
    mode: &str,
    seen: &mut HashSet<String>,
) -> anyhow::Result<Color> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("none") || t.eq_ignore_ascii_case("transparent") {
        return Ok(Color::Reset);
    }

    if let Some(hex) = t.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16)?;
            let g = u8::from_str_radix(&hex[2..4], 16)?;
            let b = u8::from_str_radix(&hex[4..6], 16)?;
            return Ok(Color::Rgb(r, g, b));
        }
    }

    // Reference resolution: defs first, then theme keys.
    if !seen.insert(t.to_string()) {
        anyhow::bail!("Theme color reference cycle detected at \"{}\"", t);
    }

    if let Some(v) = defs.get(t) {
        return resolve_color_value(json, defs, v, mode, seen);
    }
    if let Some(v) = json.theme.get(t) {
        return resolve_color_value(json, defs, v, mode, seen);
    }

    anyhow::bail!("Theme color reference \"{}\" not found", t)
}
