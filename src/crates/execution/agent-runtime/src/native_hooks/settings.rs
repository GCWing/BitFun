//! Hook settings document parsing.
//!
//! BitFun reads the same `hooks.json` document shape as Codex:
//!
//! ```json
//! {
//!   "description": "optional",
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "matcher": "Bash",
//!         "hooks": [
//!           { "type": "command", "command": "python3 check.py", "timeout": 30 }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! Validation mirrors the Codex rules: the JSON root may only contain
//! `description` and `hooks`; event names come from the fixed Codex event
//! list; unknown events are dropped with a diagnostic while valid events
//! survive; only `type: "command"` handlers are executable (`prompt` and
//! `agent` are recognized but skipped as unsupported).

use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::time::Duration;

/// Maximum size of one hooks configuration file (matches the 1 MiB Codex
/// static-inspection bound used elsewhere in this repository).
pub const MAX_HOOKS_FILE_BYTES: usize = 1024 * 1024;
/// Maximum executable handlers accepted across all configuration layers.
pub const MAX_HOOK_HANDLERS: usize = 2048;
const MAX_MATCHER_BYTES: usize = 512;

/// The fixed Codex-compatible hook event list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AgentHookEvent {
    PreToolUse,
    PermissionRequest,
    PostToolUse,
    PreCompact,
    PostCompact,
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    SubagentStart,
    SubagentStop,
    Stop,
}

impl AgentHookEvent {
    pub const ALL: [AgentHookEvent; 11] = [
        AgentHookEvent::PreToolUse,
        AgentHookEvent::PermissionRequest,
        AgentHookEvent::PostToolUse,
        AgentHookEvent::PreCompact,
        AgentHookEvent::PostCompact,
        AgentHookEvent::SessionStart,
        AgentHookEvent::SessionEnd,
        AgentHookEvent::UserPromptSubmit,
        AgentHookEvent::SubagentStart,
        AgentHookEvent::SubagentStop,
        AgentHookEvent::Stop,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            AgentHookEvent::PreToolUse => "PreToolUse",
            AgentHookEvent::PermissionRequest => "PermissionRequest",
            AgentHookEvent::PostToolUse => "PostToolUse",
            AgentHookEvent::PreCompact => "PreCompact",
            AgentHookEvent::PostCompact => "PostCompact",
            AgentHookEvent::SessionStart => "SessionStart",
            AgentHookEvent::SessionEnd => "SessionEnd",
            AgentHookEvent::UserPromptSubmit => "UserPromptSubmit",
            AgentHookEvent::SubagentStart => "SubagentStart",
            AgentHookEvent::SubagentStop => "SubagentStop",
            AgentHookEvent::Stop => "Stop",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|event| event.as_str() == name)
    }

    /// Events whose stdin payload carries `turn_id`.
    pub const fn is_turn_scoped(self) -> bool {
        !matches!(
            self,
            AgentHookEvent::SessionStart | AgentHookEvent::SessionEnd
        )
    }

    /// Default handler timeout in seconds (Codex: 600s, SessionEnd 1s).
    pub const fn default_timeout_secs(self) -> u64 {
        match self {
            AgentHookEvent::SessionEnd => 1,
            _ => 600,
        }
    }

    /// Hard cap for a configured handler timeout (Codex caps SessionEnd at 3s).
    pub const fn max_timeout_secs(self) -> Option<u64> {
        match self {
            AgentHookEvent::SessionEnd => Some(3),
            _ => None,
        }
    }

    /// Whether plain (non-JSON) stdout of a successful handler becomes
    /// model-visible context for this event.
    pub const fn plain_stdout_is_context(self) -> bool {
        matches!(
            self,
            AgentHookEvent::SessionStart
                | AgentHookEvent::UserPromptSubmit
                | AgentHookEvent::SubagentStart
        )
    }
}

impl fmt::Display for AgentHookEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a hook rule was declared. User-scope rules run before project-scope
/// rules, matching the Codex layer order (user configuration first).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentHookScope {
    User,
    Project,
}

impl AgentHookScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            AgentHookScope::User => "user",
            AgentHookScope::Project => "project",
        }
    }
}

/// One executable `type: "command"` handler.
#[derive(Debug, Clone)]
pub struct AgentHookHandler {
    pub command: String,
    /// Optional Windows override for `command` (`commandWindows`).
    pub command_windows: Option<String>,
    /// Optional timeout in seconds (`timeout`).
    pub timeout_seconds: Option<u64>,
    /// Optional UI text shown while the hook runs (`statusMessage`).
    pub status_message: Option<String>,
}

impl AgentHookHandler {
    pub fn effective_command(&self) -> &str {
        if cfg!(windows) {
            if let Some(command) = self
                .command_windows
                .as_deref()
                .map(str::trim)
                .filter(|command| !command.is_empty())
            {
                return command;
            }
        }
        &self.command
    }

    pub fn effective_timeout(&self, event: AgentHookEvent) -> Duration {
        let mut seconds = self
            .timeout_seconds
            .unwrap_or_else(|| event.default_timeout_secs());
        if seconds == 0 {
            seconds = event.default_timeout_secs();
        }
        if let Some(cap) = event.max_timeout_secs() {
            seconds = seconds.min(cap);
        }
        Duration::from_secs(seconds)
    }
}

/// Codex matcher semantics: absent, empty, or `"*"` matches everything;
/// any other string is a regular expression that must match the whole
/// matcher value (so `Bash` is an exact tool-name match, `Edit|Write`
/// matches either name, and `mcp__filesystem__.*` matches by prefix).
/// A malformed matcher never matches anything.
#[derive(Debug, Clone)]
pub enum AgentHookMatcher {
    Any,
    Pattern { raw: String, regex: Option<Regex> },
    Invalid { raw: String },
}

impl AgentHookMatcher {
    fn from_value(value: Option<&Value>) -> (Self, bool) {
        let Some(value) = value else {
            return (AgentHookMatcher::Any, true);
        };
        let Some(raw) = value.as_str() else {
            return (
                AgentHookMatcher::Invalid {
                    raw: value.to_string(),
                },
                false,
            );
        };
        if raw.is_empty() || raw == "*" {
            return (AgentHookMatcher::Any, true);
        }
        if raw.len() > MAX_MATCHER_BYTES || raw.chars().any(char::is_control) {
            return (
                AgentHookMatcher::Invalid {
                    raw: raw.to_string(),
                },
                false,
            );
        }
        let regex = Regex::new(&format!("^(?:{raw})$")).ok();
        let valid = regex.is_some();
        (
            AgentHookMatcher::Pattern {
                raw: raw.to_string(),
                regex,
            },
            valid,
        )
    }

    /// `value` is the event's matcher context (tool name, agent type,
    /// compaction trigger, or session-start source). Events without a matcher
    /// context pass `None`, which ignores configured patterns (Codex applies
    /// no filtering for those events).
    pub fn matches(&self, value: Option<&str>) -> bool {
        match self {
            AgentHookMatcher::Any => true,
            AgentHookMatcher::Pattern { regex, .. } => match (regex, value) {
                (Some(regex), Some(value)) => regex.is_match(value),
                (Some(_), None) => true,
                (None, _) => false,
            },
            AgentHookMatcher::Invalid { .. } => false,
        }
    }

    pub fn display(&self) -> &str {
        match self {
            AgentHookMatcher::Any => "*",
            AgentHookMatcher::Pattern { raw, .. } => raw,
            AgentHookMatcher::Invalid { raw } => raw,
        }
    }
}

/// One matcher group from the configuration document.
#[derive(Debug, Clone)]
pub struct AgentHookRule {
    pub matcher: AgentHookMatcher,
    pub handlers: Vec<AgentHookHandler>,
    pub scope: AgentHookScope,
    /// User-recognizable source location (for diagnostics/logs).
    pub source: String,
}

/// A configuration layer handed to [`AgentHookSettings::from_layers`].
#[derive(Debug, Clone)]
pub struct AgentHookSettingsLayer {
    pub scope: AgentHookScope,
    /// User-recognizable source location (for diagnostics/logs).
    pub source: String,
    pub bytes: Vec<u8>,
}

/// Non-fatal problems found while parsing hook settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentHookSettingsIssue {
    /// Whole document rejected: not valid JSON, not an object, or the root
    /// contains keys other than `description` and `hooks`.
    DocumentInvalid {
        source: String,
    },
    FileTooLarge {
        source: String,
    },
    EventNameUnsupported {
        source: String,
        event: String,
    },
    EventInvalid {
        source: String,
        event: String,
    },
    GroupInvalid {
        source: String,
        event: String,
    },
    HandlerInvalid {
        source: String,
        event: String,
    },
    HandlerUnsupported {
        source: String,
        event: String,
        handler_type: String,
    },
    HandlerLimitExceeded {
        source: String,
    },
    MatcherInvalid {
        source: String,
        event: String,
        matcher: String,
    },
}

impl fmt::Display for AgentHookSettingsIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentHookSettingsIssue::DocumentInvalid { source } => write!(
                f,
                "Hook configuration could not be parsed (root may only contain 'description' and 'hooks'): {source}"
            ),
            AgentHookSettingsIssue::FileTooLarge { source } => write!(
                f,
                "Hook configuration exceeds the {} byte limit: {source}",
                MAX_HOOKS_FILE_BYTES
            ),
            AgentHookSettingsIssue::EventNameUnsupported { source, event } => write!(
                f,
                "Hook event '{event}' is not a supported event name: {source}"
            ),
            AgentHookSettingsIssue::EventInvalid { source, event } => write!(
                f,
                "Hook event '{event}' must contain an array of matcher groups: {source}"
            ),
            AgentHookSettingsIssue::GroupInvalid { source, event } => write!(
                f,
                "Hook matcher group under '{event}' must be an object with a 'hooks' array: {source}"
            ),
            AgentHookSettingsIssue::HandlerInvalid { source, event } => write!(
                f,
                "Hook handler under '{event}' is missing a supported 'type' or a required field: {source}"
            ),
            AgentHookSettingsIssue::HandlerUnsupported {
                source,
                event,
                handler_type,
            } => write!(
                f,
                "Hook handler type '{handler_type}' under '{event}' is recognized but not executable by BitFun; only 'command' handlers run: {source}"
            ),
            AgentHookSettingsIssue::HandlerLimitExceeded { source } => write!(
                f,
                "Additional hook handlers were ignored after the {MAX_HOOK_HANDLERS} handler limit: {source}"
            ),
            AgentHookSettingsIssue::MatcherInvalid {
                source,
                event,
                matcher,
            } => write!(
                f,
                "Hook matcher '{matcher}' under '{event}' is not a valid pattern and will never match: {source}"
            ),
        }
    }
}

/// Parsed, merged hook settings across all configuration layers.
#[derive(Debug, Default)]
pub struct AgentHookSettings {
    rules: BTreeMap<AgentHookEvent, Vec<AgentHookRule>>,
}

impl AgentHookSettings {
    pub fn from_layers(layers: &[AgentHookSettingsLayer]) -> (Self, Vec<AgentHookSettingsIssue>) {
        let mut settings = AgentHookSettings::default();
        let mut issues = Vec::new();
        let mut remaining_handlers = MAX_HOOK_HANDLERS;
        for layer in layers {
            parse_layer(layer, &mut settings, &mut issues, &mut remaining_handlers);
        }
        (settings, issues)
    }

    pub fn is_empty(&self) -> bool {
        self.rules.values().all(|rules| rules.is_empty())
    }

    pub fn rules_for(&self, event: AgentHookEvent) -> &[AgentHookRule] {
        self.rules
            .get(&event)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn has_rules(&self, event: AgentHookEvent) -> bool {
        !self.rules_for(event).is_empty()
    }

    pub fn total_handlers(&self) -> usize {
        self.rules
            .values()
            .flatten()
            .map(|rule| rule.handlers.len())
            .sum()
    }

    /// Converts parsed command handlers into the executable registrations used
    /// by the shared runtime registry. Parsing remains independent from
    /// registry publication so callers can inspect issues before publishing.
    #[cfg(feature = "native-hook-runtime")]
    pub fn registrations(&self) -> Vec<crate::native_hooks::RuntimeHookRegistration> {
        let mut registrations = Vec::new();
        for (event, rules) in &self.rules {
            for (rule_index, rule) in rules.iter().enumerate() {
                let source = match rule.scope {
                    AgentHookScope::User => crate::native_hooks::RuntimeHookSource::UserCommand,
                    AgentHookScope::Project => {
                        crate::native_hooks::RuntimeHookSource::ProjectCommand
                    }
                };
                for (handler_index, handler) in rule.handlers.iter().enumerate() {
                    let id = format!(
                        "command.{}.{}.{}",
                        event.as_str(),
                        rule_index,
                        handler_index
                    );
                    registrations.push(crate::native_hooks::RuntimeHookRegistration::command(
                        id,
                        crate::native_hooks::RuntimeHookKind::Lifecycle(*event),
                        source,
                        handler.clone(),
                        rule.matcher.clone(),
                    ));
                }
            }
        }
        registrations
    }
}

fn parse_layer(
    layer: &AgentHookSettingsLayer,
    settings: &mut AgentHookSettings,
    issues: &mut Vec<AgentHookSettingsIssue>,
    remaining_handlers: &mut usize,
) {
    if layer.bytes.len() > MAX_HOOKS_FILE_BYTES {
        issues.push(AgentHookSettingsIssue::FileTooLarge {
            source: layer.source.clone(),
        });
        return;
    }
    let Ok(root) = serde_json::from_slice::<Value>(&layer.bytes) else {
        issues.push(AgentHookSettingsIssue::DocumentInvalid {
            source: layer.source.clone(),
        });
        return;
    };
    let Value::Object(root) = root else {
        issues.push(AgentHookSettingsIssue::DocumentInvalid {
            source: layer.source.clone(),
        });
        return;
    };
    // Codex rejects the whole hooks.json document when the root carries any
    // key besides `description` and `hooks`.
    if root
        .keys()
        .any(|key| key != "description" && key != "hooks")
    {
        issues.push(AgentHookSettingsIssue::DocumentInvalid {
            source: layer.source.clone(),
        });
        return;
    }
    let Some(events) = root.get("hooks") else {
        return;
    };
    let Value::Object(events) = events else {
        issues.push(AgentHookSettingsIssue::DocumentInvalid {
            source: layer.source.clone(),
        });
        return;
    };
    for (event_name, groups) in events {
        // `hooks.state` is a reserved Codex table, never an event.
        if event_name == "state" {
            continue;
        }
        let Some(event) = AgentHookEvent::parse(event_name) else {
            issues.push(AgentHookSettingsIssue::EventNameUnsupported {
                source: layer.source.clone(),
                event: event_name.clone(),
            });
            continue;
        };
        let Value::Array(groups) = groups else {
            issues.push(AgentHookSettingsIssue::EventInvalid {
                source: layer.source.clone(),
                event: event_name.clone(),
            });
            continue;
        };
        for group in groups {
            let Value::Object(group) = group else {
                issues.push(AgentHookSettingsIssue::GroupInvalid {
                    source: layer.source.clone(),
                    event: event_name.clone(),
                });
                continue;
            };
            let Some(Value::Array(handlers)) = group.get("hooks") else {
                issues.push(AgentHookSettingsIssue::GroupInvalid {
                    source: layer.source.clone(),
                    event: event_name.clone(),
                });
                continue;
            };
            let (matcher, matcher_valid) = AgentHookMatcher::from_value(group.get("matcher"));
            if !matcher_valid {
                issues.push(AgentHookSettingsIssue::MatcherInvalid {
                    source: layer.source.clone(),
                    event: event_name.clone(),
                    matcher: matcher.display().to_string(),
                });
            }
            let mut parsed_handlers = Vec::new();
            for handler in handlers {
                if *remaining_handlers == 0 {
                    if !issues.iter().any(|issue| {
                        matches!(
                            issue,
                            AgentHookSettingsIssue::HandlerLimitExceeded { source }
                                if *source == layer.source
                        )
                    }) {
                        issues.push(AgentHookSettingsIssue::HandlerLimitExceeded {
                            source: layer.source.clone(),
                        });
                    }
                    break;
                }
                match parse_handler(handler) {
                    ParsedHandler::Command(parsed) => {
                        *remaining_handlers -= 1;
                        parsed_handlers.push(parsed);
                    }
                    ParsedHandler::Unsupported(handler_type) => {
                        *remaining_handlers = remaining_handlers.saturating_sub(1);
                        issues.push(AgentHookSettingsIssue::HandlerUnsupported {
                            source: layer.source.clone(),
                            event: event_name.clone(),
                            handler_type,
                        });
                    }
                    ParsedHandler::Invalid => {
                        *remaining_handlers = remaining_handlers.saturating_sub(1);
                        issues.push(AgentHookSettingsIssue::HandlerInvalid {
                            source: layer.source.clone(),
                            event: event_name.clone(),
                        });
                    }
                }
            }
            if parsed_handlers.is_empty() {
                continue;
            }
            settings
                .rules
                .entry(event)
                .or_default()
                .push(AgentHookRule {
                    matcher: matcher.clone(),
                    handlers: parsed_handlers,
                    scope: layer.scope,
                    source: layer.source.clone(),
                });
        }
    }
}

enum ParsedHandler {
    Command(AgentHookHandler),
    Unsupported(String),
    Invalid,
}

fn parse_handler(handler: &Value) -> ParsedHandler {
    let Value::Object(handler) = handler else {
        return ParsedHandler::Invalid;
    };
    let Some(handler_type) = handler.get("type").and_then(Value::as_str) else {
        return ParsedHandler::Invalid;
    };
    match handler_type {
        "command" => {}
        // Codex recognizes prompt/agent declarations but they are not
        // native command handlers; BitFun skips them the same way.
        "prompt" | "agent" => return ParsedHandler::Unsupported(handler_type.to_string()),
        _ => return ParsedHandler::Invalid,
    }
    let Some(command) = handler
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
    else {
        return ParsedHandler::Invalid;
    };
    let timeout_seconds = match handler.get("timeout") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_u64().filter(|timeout| *timeout > 0) {
            Some(timeout) => Some(timeout),
            None => return ParsedHandler::Invalid,
        },
    };
    let command_windows = match handler.get("commandWindows") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_str() {
            Some(command) => Some(command.to_string()),
            None => return ParsedHandler::Invalid,
        },
    };
    let status_message = match handler.get("statusMessage") {
        None | Some(Value::Null) => None,
        Some(value) => match value.as_str() {
            Some(message) => Some(message.to_string()),
            None => return ParsedHandler::Invalid,
        },
    };
    ParsedHandler::Command(AgentHookHandler {
        command: command.to_string(),
        command_windows,
        timeout_seconds,
        status_message,
    })
}
