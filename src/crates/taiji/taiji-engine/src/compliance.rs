//! Compliance & disclaimer system — R6.3.
//!
//! Ensures every signal and AI-generated content carries explicit risk warnings.
//! The CLI requires explicit acknowledgement before any pipeline execution.

use crate::types::signal::Signal;

// ── Constants ──────────────────────────────────────────────────────────

/// Full risk disclaimer shown at CLI startup.
pub const RISK_DISCLAIMER: &str = "\
========================================
     风险揭示与免责声明
========================================
1. 本系统为个人量化研究工具，不构成投资建议。
2. 期货交易风险极高，可能导致全部本金亏损。
3. 历史回测结果不代表未来表现。
4. AI 生成的所有交易建议需经人工复核确认。
5. 系统开发者不对任何交易损失承担责任。
========================================
";

/// Default AI content label appended to generated text.
pub const DEFAULT_AI_LABEL: &str = "[AI 生成，不构成投资建议]";

/// Default disclaimer text injected into every Signal.
pub const DEFAULT_SIGNAL_DISCLAIMER: &str = "以上信号由 AI 辅助生成，不构成投资建议";

// ── Config ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ComplianceConfig {
    pub disclaimer_enabled: bool,
    pub ai_content_label: String,
    pub risk_warning_text: String,
    pub mini_app_warning: String,
}

impl Default for ComplianceConfig {
    fn default() -> Self {
        Self {
            disclaimer_enabled: true,
            ai_content_label: DEFAULT_AI_LABEL.to_string(),
            risk_warning_text: RISK_DISCLAIMER.to_string(),
            mini_app_warning: "本 MiniApp 内容为 AI 辅助生成，仅供研究参考".to_string(),
        }
    }
}

// ── Public API ─────────────────────────────────────────────────────────

/// Show the risk disclaimer and wait for user acknowledgement.
///
/// Returns `true` if the user types `"agree"` (case-sensitive).
/// Prints the disclaimer to stderr so it appears even when stdout is redirected.
pub fn show_risk_disclaimer() -> bool {
    eprintln!("{}", RISK_DISCLAIMER);
    eprint!("输入 'agree' 确认已知晓以上风险：");

    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(_) => input.trim() == "agree",
        Err(_) => false,
    }
}

/// Inject the standard signal disclaimer into a [`Signal`].
pub fn append_disclaimer(signal: &mut Signal) {
    signal.disclaimer = Some(DEFAULT_SIGNAL_DISCLAIMER.to_string());
}

/// Append the AI content label to an arbitrary text string.
///
/// ```
/// let result = taiji_engine::compliance::label_ai_content("明日重点关注螺纹钢 MA 交叉信号");
/// assert!(result.contains("[AI 生成，不构成投资建议]"));
/// ```
pub fn label_ai_content(text: &str) -> String {
    format!("{} {}", text, DEFAULT_AI_LABEL)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::bar::Freq;
    use crate::types::signal::{Signal, SignalAction};
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn label_ai_content_appends_label() {
        let original = "明日重点关注螺纹钢 MA 交叉信号";
        let labeled = label_ai_content(original);
        assert!(labeled.starts_with(original));
        assert!(labeled.contains(DEFAULT_AI_LABEL));
    }

    #[test]
    fn label_ai_content_empty_input() {
        let labeled = label_ai_content("");
        assert_eq!(labeled, " [AI 生成，不构成投资建议]");
    }

    #[test]
    fn append_disclaimer_sets_field() {
        let mut signal = Signal {
            timestamp: Utc::now(),
            instrument: "rb2510".into(),
            freq: Freq::D,
            action: SignalAction::Hold,
            entry: None,
            stop_loss: None,
            take_profit: None,
            size: None,
            source: "test".into(),
            confidence: 0.0,
            metadata: HashMap::new(),
            disclaimer: None,
        };

        append_disclaimer(&mut signal);
        assert_eq!(
            signal.disclaimer.as_deref(),
            Some(DEFAULT_SIGNAL_DISCLAIMER)
        );
    }

    #[test]
    fn compliance_config_defaults() {
        let cfg = ComplianceConfig::default();
        assert!(cfg.disclaimer_enabled);
        assert_eq!(cfg.ai_content_label, DEFAULT_AI_LABEL);
        assert_eq!(cfg.risk_warning_text, RISK_DISCLAIMER);
    }
}
