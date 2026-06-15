//! MiniApp per-app rate-limit rules shared by host bridge surfaces.

use std::collections::HashMap;

const RATE_LIMIT_WINDOW_MS: u64 = 60_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MiniAppRateLimitSubject {
    Ai,
    Agent,
}

impl MiniAppRateLimitSubject {
    fn label(self) -> &'static str {
        match self {
            Self::Ai => "AI",
            Self::Agent => "Agent",
        }
    }

    fn unit(self) -> &'static str {
        match self {
            Self::Ai => "requests",
            Self::Agent => "runs",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MiniAppRateLimitState {
    entries: HashMap<String, (u32, u64)>,
}

impl MiniAppRateLimitState {
    pub fn check(
        &mut self,
        app_id: &str,
        rate_limit_per_minute: u32,
        now_ms: u64,
        subject: MiniAppRateLimitSubject,
    ) -> Result<(), String> {
        if rate_limit_per_minute == 0 {
            return Ok(());
        }
        let entry = self
            .entries
            .entry(app_id.to_string())
            .or_insert((0, now_ms));
        if now_ms.saturating_sub(entry.1) >= RATE_LIMIT_WINDOW_MS {
            *entry = (1, now_ms);
        } else {
            entry.0 += 1;
            if entry.0 > rate_limit_per_minute {
                return Err(format!(
                    "{} rate limit exceeded: max {} {}/minute",
                    subject.label(),
                    rate_limit_per_minute,
                    subject.unit()
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_preserves_ai_and_agent_error_contracts() {
        let mut state = MiniAppRateLimitState::default();
        state
            .check("app", 2, 1000, MiniAppRateLimitSubject::Ai)
            .unwrap();
        state
            .check("app", 2, 2000, MiniAppRateLimitSubject::Ai)
            .unwrap();
        assert_eq!(
            state
                .check("app", 2, 3000, MiniAppRateLimitSubject::Ai)
                .unwrap_err(),
            "AI rate limit exceeded: max 2 requests/minute"
        );

        let mut state = MiniAppRateLimitState::default();
        state
            .check("app", 1, 1000, MiniAppRateLimitSubject::Agent)
            .unwrap();
        assert_eq!(
            state
                .check("app", 1, 2000, MiniAppRateLimitSubject::Agent)
                .unwrap_err(),
            "Agent rate limit exceeded: max 1 runs/minute"
        );
    }

    #[test]
    fn rate_limit_window_resets_after_one_minute() {
        let mut state = MiniAppRateLimitState::default();
        state
            .check("app", 1, 1000, MiniAppRateLimitSubject::Ai)
            .unwrap();
        state
            .check("app", 1, 61_000, MiniAppRateLimitSubject::Ai)
            .unwrap();
    }
}
