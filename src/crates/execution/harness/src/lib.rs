//! Harness workflow contracts.
//!
//! This crate owns provider-neutral workflow descriptors and registry wiring.
//! Concrete workflow execution remains in product/runtime owners until it can
//! be moved behind explicit ports without changing behavior.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HarnessWorkflow {
    Sdd,
    DeepReview,
    DeepResearch,
    MiniApp,
    FunctionAgent,
}

impl HarnessWorkflow {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sdd => "sdd",
            Self::DeepReview => "deep-review",
            Self::DeepResearch => "deep-research",
            Self::MiniApp => "miniapp",
            Self::FunctionAgent => "function-agent",
        }
    }
}

impl fmt::Display for HarnessWorkflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HarnessCapability {
    Plan,
    Execute,
    ReviewGate,
    Artifact,
    PostProcessor,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HarnessId(String);

impl HarnessId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HarnessId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarnessPlanningContext {
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarnessExecutionContext {
    pub request_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessInput {
    workflow: HarnessWorkflow,
    goal: String,
}

impl HarnessInput {
    pub fn new(workflow: HarnessWorkflow, goal: impl Into<String>) -> Self {
        Self {
            workflow,
            goal: goal.into(),
        }
    }

    pub fn workflow(&self) -> HarnessWorkflow {
        self.workflow
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessStepKind {
    LegacyFacade,
    AgentRuntime,
    ToolRuntime,
    RuntimeService,
    ProductDomain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessStep {
    id: String,
    kind: HarnessStepKind,
    target: String,
}

impl HarnessStep {
    pub fn new(id: impl Into<String>, kind: HarnessStepKind, target: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            kind,
            target: target.into(),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn kind(&self) -> HarnessStepKind {
        self.kind
    }

    pub fn target(&self) -> &str {
        &self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessPlan {
    provider_id: HarnessId,
    workflow: HarnessWorkflow,
    goal: String,
    steps: Vec<HarnessStep>,
}

impl HarnessPlan {
    pub fn new(
        provider_id: HarnessId,
        workflow: HarnessWorkflow,
        goal: impl Into<String>,
        steps: Vec<HarnessStep>,
    ) -> Self {
        Self {
            provider_id,
            workflow,
            goal: goal.into(),
            steps,
        }
    }

    pub fn provider_id(&self) -> &HarnessId {
        &self.provider_id
    }

    pub fn workflow(&self) -> HarnessWorkflow {
        self.workflow
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn steps(&self) -> &[HarnessStep] {
        &self.steps
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarnessOutcomeStatus {
    Completed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessOutcome {
    status: HarnessOutcomeStatus,
}

impl HarnessOutcome {
    pub fn new(status: HarnessOutcomeStatus) -> Self {
        Self { status }
    }

    pub fn status(&self) -> HarnessOutcomeStatus {
        self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HarnessError {
    #[error("provider {provider_id} does not support workflow {requested}; supported workflow is {supported}")]
    UnsupportedWorkflow {
        provider_id: HarnessId,
        requested: HarnessWorkflow,
        supported: HarnessWorkflow,
    },
    #[error("provider {provider_id} does not execute workflow {workflow}: {reason}")]
    UnsupportedExecution {
        provider_id: HarnessId,
        workflow: HarnessWorkflow,
        reason: String,
    },
}

#[async_trait]
pub trait HarnessProvider: Send + Sync {
    fn id(&self) -> &HarnessId;

    fn workflow(&self) -> HarnessWorkflow;

    fn capabilities(&self) -> &[HarnessCapability];

    async fn plan(
        &self,
        ctx: HarnessPlanningContext,
        input: HarnessInput,
    ) -> Result<HarnessPlan, HarnessError>;

    async fn execute(
        &self,
        ctx: HarnessExecutionContext,
        plan: HarnessPlan,
    ) -> Result<HarnessOutcome, HarnessError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptorHarnessProvider {
    id: HarnessId,
    workflow: HarnessWorkflow,
    capabilities: Vec<HarnessCapability>,
    legacy_target: String,
}

impl DescriptorHarnessProvider {
    pub fn legacy_facade(
        id: impl Into<String>,
        workflow: HarnessWorkflow,
        capabilities: &[HarnessCapability],
        legacy_target: impl Into<String>,
    ) -> Self {
        Self {
            id: HarnessId::new(id),
            workflow,
            capabilities: capabilities
                .iter()
                .copied()
                .filter(|capability| *capability != HarnessCapability::Execute)
                .collect(),
            legacy_target: legacy_target.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HarnessProviderDescriptor {
    provider_id: &'static str,
    workflow: HarnessWorkflow,
    capabilities: &'static [HarnessCapability],
    legacy_target: &'static str,
}

impl HarnessProviderDescriptor {
    pub const fn legacy_facade(
        provider_id: &'static str,
        workflow: HarnessWorkflow,
        capabilities: &'static [HarnessCapability],
        legacy_target: &'static str,
    ) -> Self {
        Self {
            provider_id,
            workflow,
            capabilities,
            legacy_target,
        }
    }

    pub const fn provider_id(self) -> &'static str {
        self.provider_id
    }

    pub const fn workflow(self) -> HarnessWorkflow {
        self.workflow
    }

    pub const fn capabilities(self) -> &'static [HarnessCapability] {
        self.capabilities
    }

    pub const fn legacy_target(self) -> &'static str {
        self.legacy_target
    }

    pub fn into_provider(self) -> DescriptorHarnessProvider {
        DescriptorHarnessProvider::legacy_facade(
            self.provider_id,
            self.workflow,
            self.capabilities,
            self.legacy_target,
        )
    }
}

pub fn build_descriptor_harness_registry<I>(
    descriptors: I,
) -> Result<HarnessRegistry, HarnessRegistryBuildError>
where
    I: IntoIterator<Item = HarnessProviderDescriptor>,
{
    let mut builder = HarnessRegistryBuilder::new();
    for descriptor in descriptors {
        builder = builder.install_provider(descriptor.into_provider());
    }
    builder.build()
}

#[async_trait]
impl HarnessProvider for DescriptorHarnessProvider {
    fn id(&self) -> &HarnessId {
        &self.id
    }

    fn workflow(&self) -> HarnessWorkflow {
        self.workflow
    }

    fn capabilities(&self) -> &[HarnessCapability] {
        &self.capabilities
    }

    async fn plan(
        &self,
        _ctx: HarnessPlanningContext,
        input: HarnessInput,
    ) -> Result<HarnessPlan, HarnessError> {
        if input.workflow() != self.workflow {
            return Err(HarnessError::UnsupportedWorkflow {
                provider_id: self.id.clone(),
                requested: input.workflow(),
                supported: self.workflow,
            });
        }

        Ok(HarnessPlan::new(
            self.id.clone(),
            self.workflow,
            input.goal(),
            vec![HarnessStep::new(
                format!("{}.legacy_facade", self.workflow.id()),
                HarnessStepKind::LegacyFacade,
                self.legacy_target.clone(),
            )],
        ))
    }

    async fn execute(
        &self,
        _ctx: HarnessExecutionContext,
        plan: HarnessPlan,
    ) -> Result<HarnessOutcome, HarnessError> {
        Err(HarnessError::UnsupportedExecution {
            provider_id: self.id.clone(),
            workflow: plan.workflow(),
            reason: "concrete execution remains on the legacy product path".to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HarnessRegistryBuildError {
    #[error("duplicate harness provider id {provider_id}")]
    DuplicateProviderId { provider_id: HarnessId },
}

#[derive(Default)]
pub struct HarnessRegistryBuilder {
    providers: Vec<Arc<dyn HarnessProvider>>,
}

impl HarnessRegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_provider<P>(mut self, provider: P) -> Self
    where
        P: HarnessProvider + 'static,
    {
        self.providers.push(Arc::new(provider));
        self
    }

    pub fn build(self) -> Result<HarnessRegistry, HarnessRegistryBuildError> {
        let mut provider_ids = HashSet::new();
        for provider in &self.providers {
            if !provider_ids.insert(provider.id().clone()) {
                return Err(HarnessRegistryBuildError::DuplicateProviderId {
                    provider_id: provider.id().clone(),
                });
            }
        }

        Ok(HarnessRegistry {
            providers: self.providers,
        })
    }
}

#[derive(Default)]
pub struct HarnessRegistry {
    providers: Vec<Arc<dyn HarnessProvider>>,
}

impl fmt::Debug for HarnessRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HarnessRegistry")
            .field("provider_ids", &self.provider_ids())
            .field("workflows", &self.workflows())
            .finish()
    }
}

impl HarnessRegistry {
    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers
            .iter()
            .map(|provider| provider.id().as_str())
            .collect()
    }

    pub fn workflows(&self) -> Vec<HarnessWorkflow> {
        let mut workflows = Vec::new();
        for provider in &self.providers {
            let workflow = provider.workflow();
            if !workflows.contains(&workflow) {
                workflows.push(workflow);
            }
        }
        workflows
    }

    pub fn provider_for_workflow(&self, workflow: HarnessWorkflow) -> Option<&dyn HarnessProvider> {
        self.providers
            .iter()
            .find(|provider| provider.workflow() == workflow)
            .map(|provider| provider.as_ref())
    }

    pub fn provider_by_id(&self, provider_id: &str) -> Option<&dyn HarnessProvider> {
        self.providers
            .iter()
            .find(|provider| provider.id().as_str() == provider_id)
            .map(|provider| provider.as_ref())
    }
}
