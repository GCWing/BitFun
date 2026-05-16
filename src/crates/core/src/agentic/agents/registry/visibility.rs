use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuiltinSubagentExposure {
    Public,
    Restricted,
    Hidden,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentVisibilitySummary {
    pub exposure: BuiltinSubagentExposure,
    pub allowed_parent_agent_ids: Vec<String>,
    pub denied_parent_agent_ids: Vec<String>,
    pub show_in_global_registry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentVisibilityPolicy {
    pub exposure: BuiltinSubagentExposure,
    pub allowed_parent_agent_ids: HashSet<String>,
    pub denied_parent_agent_ids: HashSet<String>,
    pub show_in_global_registry: bool,
}

impl SubagentVisibilityPolicy {
    pub fn public() -> Self {
        Self {
            exposure: BuiltinSubagentExposure::Public,
            allowed_parent_agent_ids: HashSet::new(),
            denied_parent_agent_ids: HashSet::new(),
            show_in_global_registry: true,
        }
    }

    pub fn restricted<I, S>(allowed_parent_agent_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            exposure: BuiltinSubagentExposure::Restricted,
            allowed_parent_agent_ids: allowed_parent_agent_ids
                .into_iter()
                .map(Into::into)
                .collect(),
            denied_parent_agent_ids: HashSet::new(),
            show_in_global_registry: true,
        }
    }

    pub fn hidden<I, S>(allowed_parent_agent_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            exposure: BuiltinSubagentExposure::Hidden,
            allowed_parent_agent_ids: allowed_parent_agent_ids
                .into_iter()
                .map(Into::into)
                .collect(),
            denied_parent_agent_ids: HashSet::new(),
            show_in_global_registry: false,
        }
    }

    pub fn deny_for<I, S>(mut self, denied_parent_agent_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.denied_parent_agent_ids = denied_parent_agent_ids
            .into_iter()
            .map(Into::into)
            .collect();
        self
    }

    pub fn summary(&self) -> SubagentVisibilitySummary {
        let mut allowed_parent_agent_ids: Vec<String> =
            self.allowed_parent_agent_ids.iter().cloned().collect();
        allowed_parent_agent_ids.sort();

        let mut denied_parent_agent_ids: Vec<String> =
            self.denied_parent_agent_ids.iter().cloned().collect();
        denied_parent_agent_ids.sort();

        SubagentVisibilitySummary {
            exposure: self.exposure,
            allowed_parent_agent_ids,
            denied_parent_agent_ids,
            show_in_global_registry: self.show_in_global_registry,
        }
    }

    pub fn can_access_from_parent(&self, parent_agent_type: Option<&str>) -> bool {
        let normalized_parent = parent_agent_type
            .map(str::trim)
            .filter(|value| !value.is_empty());

        if normalized_parent.is_some_and(|parent| self.denied_parent_agent_ids.contains(parent)) {
            return false;
        }

        match self.exposure {
            BuiltinSubagentExposure::Public => true,
            BuiltinSubagentExposure::Restricted | BuiltinSubagentExposure::Hidden => normalized_parent
                .is_some_and(|parent| self.allowed_parent_agent_ids.contains(parent)),
        }
    }
}

impl Default for SubagentVisibilityPolicy {
    fn default() -> Self {
        Self::public()
    }
}
