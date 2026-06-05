use agent_client_protocol::schema::{
    Cost, ModelInfo, SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOptions, SessionModelState,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionContextUsage {
    pub used: u64,
    pub size: u64,
    #[serde(default)]
    pub cost: Option<Cost>,
}

impl From<agent_client_protocol::schema::UsageUpdate> for AcpSessionContextUsage {
    fn from(update: agent_client_protocol::schema::UsageUpdate) -> Self {
        Self {
            used: update.used,
            size: update.size,
            cost: update.cost,
        }
    }
}

/// A slash command advertised by an ACP agent via `AvailableCommandsUpdate`.
///
/// Invocation is plain prompt text: `/<name> <args>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpAvailableCommand {
    pub name: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_hint: Option<String>,
}

impl From<agent_client_protocol::schema::AvailableCommand> for AcpAvailableCommand {
    fn from(command: agent_client_protocol::schema::AvailableCommand) -> Self {
        use agent_client_protocol::schema::AvailableCommandInput;

        let input_hint = command.input.and_then(|input| match input {
            AvailableCommandInput::Unstructured(unstructured) => Some(unstructured.hint),
            _ => None,
        });

        Self {
            name: command.name,
            description: command.description,
            input_hint,
        }
    }
}

/// One entry of an ACP agent execution plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpPlanEntry {
    pub content: String,
    pub priority: String,
    pub status: String,
}

impl From<agent_client_protocol::schema::PlanEntry> for AcpPlanEntry {
    fn from(entry: agent_client_protocol::schema::PlanEntry) -> Self {
        use agent_client_protocol::schema::{PlanEntryPriority, PlanEntryStatus};

        let priority = match entry.priority {
            PlanEntryPriority::High => "high",
            PlanEntryPriority::Medium => "medium",
            PlanEntryPriority::Low => "low",
            _ => "medium",
        };
        let status = match entry.status {
            PlanEntryStatus::Pending => "pending",
            PlanEntryStatus::InProgress => "in_progress",
            PlanEntryStatus::Completed => "completed",
            _ => "pending",
        };

        Self {
            content: entry.content,
            priority: priority.to_string(),
            status: status.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionOptions {
    #[serde(default)]
    pub current_model_id: Option<String>,
    #[serde(default)]
    pub available_models: Vec<AcpSessionModelOption>,
    #[serde(default)]
    pub model_config_id: Option<String>,
    #[serde(default)]
    pub context_usage: Option<AcpSessionContextUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcpSessionModelOption {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

pub(super) fn session_options_from_state(
    models: Option<&SessionModelState>,
    config_options: &[SessionConfigOption],
    context_usage: Option<&AcpSessionContextUsage>,
) -> AcpSessionOptions {
    let context_usage = context_usage.cloned();
    if let Some(models) = models.filter(|models| !models.available_models.is_empty()) {
        return AcpSessionOptions {
            current_model_id: Some(models.current_model_id.to_string()),
            available_models: models
                .available_models
                .iter()
                .map(model_option_from_model_info)
                .collect(),
            model_config_id: None,
            context_usage,
        };
    }

    if let Some(option) = model_config_option(config_options) {
        let (current_model_id, available_models) = select_model_values(option);
        return AcpSessionOptions {
            current_model_id,
            available_models,
            model_config_id: Some(option.id.to_string()),
            context_usage,
        };
    }

    AcpSessionOptions {
        context_usage,
        ..Default::default()
    }
}

pub(super) fn model_config_id(config_options: &[SessionConfigOption]) -> Option<String> {
    model_config_option(config_options).map(|option| option.id.to_string())
}

fn model_option_from_model_info(model: &ModelInfo) -> AcpSessionModelOption {
    AcpSessionModelOption {
        id: model.model_id.to_string(),
        name: model.name.clone(),
        description: model.description.clone(),
    }
}

fn model_config_option(config_options: &[SessionConfigOption]) -> Option<&SessionConfigOption> {
    config_options
        .iter()
        .find(|option| matches!(option.category, Some(SessionConfigOptionCategory::Model)))
        .or_else(|| {
            config_options.iter().find(|option| {
                let id = option.id.to_string().to_ascii_lowercase();
                let name = option.name.to_ascii_lowercase();
                id == "model" || id.ends_with("_model") || name.contains("model")
            })
        })
        .filter(|option| matches!(option.kind, SessionConfigKind::Select(_)))
}

fn select_model_values(
    option: &SessionConfigOption,
) -> (Option<String>, Vec<AcpSessionModelOption>) {
    let SessionConfigKind::Select(select) = &option.kind else {
        return (None, Vec::new());
    };

    let models = match &select.options {
        SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .map(|option| AcpSessionModelOption {
                id: option.value.to_string(),
                name: option.name.clone(),
                description: option.description.clone(),
            })
            .collect(),
        SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| {
                group.options.iter().map(|option| AcpSessionModelOption {
                    id: option.value.to_string(),
                    name: option.name.clone(),
                    description: option.description.clone(),
                })
            })
            .collect(),
        _ => Vec::new(),
    };

    (Some(select.current_value.to_string()), models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::{ModelInfo, SessionConfigOption};

    #[test]
    fn converts_native_model_state() {
        let state = SessionModelState::new("gpt-5.4", vec![ModelInfo::new("gpt-5.4", "GPT 5.4")]);

        let options = session_options_from_state(Some(&state), &[], None);

        assert_eq!(options.current_model_id.as_deref(), Some("gpt-5.4"));
        assert_eq!(options.available_models.len(), 1);
        assert_eq!(options.available_models[0].name, "GPT 5.4");
        assert!(options.model_config_id.is_none());
    }

    #[test]
    fn converts_model_config_option_fallback() {
        let config = SessionConfigOption::select(
            "model",
            "Model",
            "fast",
            vec![
                agent_client_protocol::schema::SessionConfigSelectOption::new("fast", "Fast"),
                agent_client_protocol::schema::SessionConfigSelectOption::new("smart", "Smart"),
            ],
        )
        .category(SessionConfigOptionCategory::Model);

        let options = session_options_from_state(None, &[config], None);

        assert_eq!(options.current_model_id.as_deref(), Some("fast"));
        assert_eq!(options.model_config_id.as_deref(), Some("model"));
        assert_eq!(options.available_models.len(), 2);
        assert_eq!(options.available_models[1].id, "smart");
    }

    #[test]
    fn includes_context_usage() {
        let state = SessionModelState::new("gpt-5.4", vec![ModelInfo::new("gpt-5.4", "GPT 5.4")]);
        let usage = AcpSessionContextUsage {
            used: 42_000,
            size: 128_000,
            cost: Some(agent_client_protocol::schema::Cost::new(0.12, "USD")),
        };

        let options = session_options_from_state(Some(&state), &[], Some(&usage));

        let context_usage = options.context_usage.expect("context usage");
        assert_eq!(context_usage.used, 42_000);
        assert_eq!(context_usage.size, 128_000);
        assert_eq!(
            context_usage
                .cost
                .as_ref()
                .map(|cost| cost.currency.as_str()),
            Some("USD")
        );
    }

    #[test]
    fn converts_available_command_with_and_without_input() {
        use agent_client_protocol::schema::{
            AvailableCommand, AvailableCommandInput, UnstructuredCommandInput,
        };

        let with_input = AvailableCommand::new("create_plan", "Draft an execution plan").input(
            AvailableCommandInput::Unstructured(UnstructuredCommandInput::new("what to plan")),
        );
        let converted = AcpAvailableCommand::from(with_input);
        assert_eq!(converted.name, "create_plan");
        assert_eq!(converted.description, "Draft an execution plan");
        assert_eq!(converted.input_hint.as_deref(), Some("what to plan"));

        let converted =
            AcpAvailableCommand::from(AvailableCommand::new("compact", "Compact the context"));
        assert_eq!(converted.name, "compact");
        assert!(converted.input_hint.is_none());
    }
}
