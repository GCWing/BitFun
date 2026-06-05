use agent_client_protocol::schema::AgentCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AcpRemoteSessionStrategy {
    New,
    Load,
    Resume,
}

impl AcpRemoteSessionStrategy {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Load => "load",
            Self::Resume => "resume",
        }
    }

    pub(super) fn startup_phase_name(self) -> &'static str {
        match self {
            Self::New => "session creation",
            Self::Load | Self::Resume => "session restore",
        }
    }
}

pub(super) fn preferred_resume_strategies(
    capabilities: Option<&AgentCapabilities>,
    remote_session_id: Option<&str>,
) -> Vec<AcpRemoteSessionStrategy> {
    let mut strategies = Vec::new();
    let has_remote_session_id = remote_session_id
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    if has_remote_session_id {
        // Prefer loading saved session state over resuming a live stream. Some
        // ACP clients continue an unfinished prompt on resume, and ACP update
        // notifications are only scoped to the remote session, not a BitFun turn.
        if capabilities
            .map(|capabilities| capabilities.load_session)
            .unwrap_or(false)
        {
            strategies.push(AcpRemoteSessionStrategy::Load);
        }

        if capabilities
            .and_then(|capabilities| capabilities.session_capabilities.resume.as_ref())
            .is_some()
        {
            strategies.push(AcpRemoteSessionStrategy::Resume);
        }
    }

    strategies.push(AcpRemoteSessionStrategy::New);
    strategies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_new_without_remote_session_id() {
        assert_eq!(
            preferred_resume_strategies(Some(&AgentCapabilities::new().load_session(true)), None),
            vec![AcpRemoteSessionStrategy::New]
        );
    }

    #[test]
    fn prefers_load_when_resume_is_not_supported() {
        assert_eq!(
            preferred_resume_strategies(
                Some(&AgentCapabilities::new().load_session(true)),
                Some("s1")
            ),
            vec![
                AcpRemoteSessionStrategy::Load,
                AcpRemoteSessionStrategy::New
            ]
        );
    }

    #[test]
    fn prefers_load_before_resume_when_both_are_supported() {
        assert_eq!(
            preferred_resume_strategies(
                Some(
                    &AgentCapabilities::new()
                        .load_session(true)
                        .session_capabilities(
                            agent_client_protocol::schema::SessionCapabilities::new().resume(
                                agent_client_protocol::schema::SessionResumeCapabilities::new(),
                            ),
                        ),
                ),
                Some("s1")
            ),
            vec![
                AcpRemoteSessionStrategy::Load,
                AcpRemoteSessionStrategy::Resume,
                AcpRemoteSessionStrategy::New
            ]
        );
    }
}
