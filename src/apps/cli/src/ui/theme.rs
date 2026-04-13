/// Theme and style definitions
use ratatui::style::{Color, Modifier, Style};

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
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            primary: Color::Rgb(59, 130, 246),  // blue
            success: Color::Rgb(34, 197, 94),   // green
            warning: Color::Rgb(251, 191, 36),  // yellow
            error: Color::Rgb(239, 68, 68),     // red
            info: Color::Rgb(147, 197, 253),    // light blue
            muted: Color::Rgb(156, 163, 175),   // gray
            background: Color::Rgb(17, 24, 39), // dark gray background
            border: Color::Rgb(55, 65, 81),     // border gray
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
        }
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
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StyleKind {
    Primary,
    Success,
    Warning,
    Error,
    Info,
    Muted,
    Title,
    Border,
}

pub fn tool_icon(tool_name: &str) -> (&'static str, Color) {
    match tool_name {
        "FileReadTool" => ("[R]", Color::Rgb(59, 130, 246)),
        "FileWriteTool" => ("[W]", Color::Rgb(34, 197, 94)),
        "FileEditTool" => ("[E]", Color::Rgb(251, 191, 36)),
        "FileDeleteTool" => ("[D]", Color::Rgb(239, 68, 68)),
        "BashTool" | "ShellTool" => ("[!]", Color::Rgb(147, 51, 234)),
        "SearchTool" => ("[S]", Color::Rgb(59, 130, 246)),
        "AnalysisTool" => ("[A]", Color::Rgb(236, 72, 153)),
        _ => ("[T]", Color::Rgb(156, 163, 175)),
    }
}
