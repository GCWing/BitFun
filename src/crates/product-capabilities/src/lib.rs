//! Product capability pack contracts.
//!
//! This crate owns provider-neutral product capability assembly facts. Concrete
//! workflow execution and tool implementations remain in their runtime owners.

use std::collections::HashSet;
use std::fmt;

use bitfun_harness::{
    DescriptorHarnessProvider, HarnessCapability, HarnessRegistry, HarnessRegistryBuildError,
    HarnessRegistryBuilder, HarnessWorkflow,
};
use bitfun_runtime_ports::RuntimeServiceCapability;
use bitfun_tool_packs::{product_tool_provider_group_plan, ToolProviderGroupPlan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductCapabilityBuildError {
    UnknownToolProviderGroup { provider_id: &'static str },
}

impl fmt::Display for ProductCapabilityBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownToolProviderGroup { provider_id } => {
                write!(f, "unknown tool provider group {provider_id}")
            }
        }
    }
}

impl std::error::Error for ProductCapabilityBuildError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProductCapabilityId {
    CodeAgent,
    DeepReview,
    DeepResearch,
    MiniApp,
}

impl ProductCapabilityId {
    pub const fn id(self) -> &'static str {
        match self {
            Self::CodeAgent => "code-agent",
            Self::DeepReview => "deep-review",
            Self::DeepResearch => "deep-research",
            Self::MiniApp => "miniapp",
        }
    }
}

impl fmt::Display for ProductCapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductCapabilityPack {
    id: ProductCapabilityId,
    required_services: &'static [RuntimeServiceCapability],
    tool_provider_group_ids: &'static [&'static str],
    harness_provider_descriptors: &'static [HarnessProviderDescriptor],
}

impl ProductCapabilityPack {
    pub const fn new(
        id: ProductCapabilityId,
        required_services: &'static [RuntimeServiceCapability],
        tool_provider_group_ids: &'static [&'static str],
        harness_provider_descriptors: &'static [HarnessProviderDescriptor],
    ) -> Self {
        Self {
            id,
            required_services,
            tool_provider_group_ids,
            harness_provider_descriptors,
        }
    }

    pub const fn id(self) -> ProductCapabilityId {
        self.id
    }

    pub const fn required_services(self) -> &'static [RuntimeServiceCapability] {
        self.required_services
    }

    pub const fn tool_provider_group_ids(self) -> &'static [&'static str] {
        self.tool_provider_group_ids
    }

    pub const fn harness_provider_descriptors(self) -> &'static [HarnessProviderDescriptor] {
        self.harness_provider_descriptors
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProductCapabilityRegistry {
    packs: &'static [ProductCapabilityPack],
}

impl ProductCapabilityRegistry {
    pub const fn new(packs: &'static [ProductCapabilityPack]) -> Self {
        Self { packs }
    }

    pub const fn packs(self) -> &'static [ProductCapabilityPack] {
        self.packs
    }

    pub fn capability_ids(self) -> Vec<ProductCapabilityId> {
        self.packs.iter().map(|pack| pack.id()).collect()
    }

    pub fn required_service_capabilities(self) -> Vec<RuntimeServiceCapability> {
        let mut seen = HashSet::new();
        let mut capabilities = Vec::new();
        for pack in self.packs {
            for capability in pack.required_services() {
                if seen.insert(*capability) {
                    capabilities.push(*capability);
                }
            }
        }
        capabilities
    }

    pub fn tool_provider_group_ids(self) -> Vec<&'static str> {
        let mut seen = HashSet::new();
        let mut provider_ids = Vec::new();
        for pack in self.packs {
            for provider_id in pack.tool_provider_group_ids() {
                if seen.insert(*provider_id) {
                    provider_ids.push(*provider_id);
                }
            }
        }
        provider_ids
    }

    pub fn try_tool_provider_group_plan(
        self,
    ) -> Result<Vec<ToolProviderGroupPlan>, ProductCapabilityBuildError> {
        let provider_ids = self.tool_provider_group_ids();
        let requested_provider_ids = provider_ids.iter().copied().collect::<HashSet<_>>();
        let mut found_provider_ids = HashSet::new();
        let mut plan = Vec::new();

        for group_plan in product_tool_provider_group_plan() {
            if requested_provider_ids.contains(group_plan.provider_id()) {
                found_provider_ids.insert(group_plan.provider_id());
                plan.push(*group_plan);
            }
        }

        for provider_id in provider_ids {
            if !found_provider_ids.contains(provider_id) {
                return Err(ProductCapabilityBuildError::UnknownToolProviderGroup { provider_id });
            }
        }

        Ok(plan)
    }

    pub fn tool_provider_group_plan(self) -> Vec<ToolProviderGroupPlan> {
        self.try_tool_provider_group_plan()
            .expect("product capability packs must reference known tool provider groups")
    }

    pub fn harness_provider_descriptors(self) -> Vec<HarnessProviderDescriptor> {
        let mut seen = HashSet::new();
        let mut descriptors = Vec::new();
        for pack in self.packs {
            for descriptor in pack.harness_provider_descriptors() {
                if seen.insert(descriptor.provider_id()) {
                    descriptors.push(*descriptor);
                }
            }
        }
        descriptors
    }

    pub fn build_harness_registry(self) -> Result<HarnessRegistry, HarnessRegistryBuildError> {
        let mut builder = HarnessRegistryBuilder::new();
        for descriptor in self.harness_provider_descriptors() {
            builder = builder.install_provider(descriptor.into_provider());
        }
        builder.build()
    }
}

const CODE_AGENT_SERVICES: &[RuntimeServiceCapability] = &[
    RuntimeServiceCapability::FileSystem,
    RuntimeServiceCapability::Workspace,
    RuntimeServiceCapability::SessionStore,
    RuntimeServiceCapability::Permission,
    RuntimeServiceCapability::Events,
    RuntimeServiceCapability::Clock,
];
const DEEP_REVIEW_SERVICES: &[RuntimeServiceCapability] = &[
    RuntimeServiceCapability::Workspace,
    RuntimeServiceCapability::Git,
    RuntimeServiceCapability::Permission,
    RuntimeServiceCapability::Events,
];
const DEEP_RESEARCH_SERVICES: &[RuntimeServiceCapability] = &[
    RuntimeServiceCapability::Workspace,
    RuntimeServiceCapability::Network,
    RuntimeServiceCapability::Permission,
    RuntimeServiceCapability::Events,
];
const MINIAPP_SERVICES: &[RuntimeServiceCapability] = &[
    RuntimeServiceCapability::FileSystem,
    RuntimeServiceCapability::Workspace,
    RuntimeServiceCapability::Permission,
    RuntimeServiceCapability::Events,
];

const CODE_AGENT_TOOL_GROUPS: &[&str] = &["core.basic", "core.agent", "core.session"];
const INTEGRATION_TOOL_GROUPS: &[&str] = &["core.integration"];

const DEEP_REVIEW_HARNESS_CAPABILITIES: &[HarnessCapability] = &[
    HarnessCapability::Plan,
    HarnessCapability::ReviewGate,
    HarnessCapability::PostProcessor,
];
const DEEP_RESEARCH_HARNESS_CAPABILITIES: &[HarnessCapability] =
    &[HarnessCapability::Plan, HarnessCapability::PostProcessor];
const MINIAPP_HARNESS_CAPABILITIES: &[HarnessCapability] =
    &[HarnessCapability::Plan, HarnessCapability::Artifact];

pub const CORE_DEEP_REVIEW_HARNESS_PROVIDER_ID: &str = "core.deep_review";
pub const CORE_DEEP_RESEARCH_HARNESS_PROVIDER_ID: &str = "core.deep_research";
pub const CORE_MINIAPP_HARNESS_PROVIDER_ID: &str = "core.miniapp";

const DEEP_REVIEW_HARNESS_PROVIDER: HarnessProviderDescriptor =
    HarnessProviderDescriptor::legacy_facade(
        CORE_DEEP_REVIEW_HARNESS_PROVIDER_ID,
        HarnessWorkflow::DeepReview,
        DEEP_REVIEW_HARNESS_CAPABILITIES,
        "bitfun-core::agentic::deep_review",
    );
const DEEP_RESEARCH_HARNESS_PROVIDER: HarnessProviderDescriptor =
    HarnessProviderDescriptor::legacy_facade(
        CORE_DEEP_RESEARCH_HARNESS_PROVIDER_ID,
        HarnessWorkflow::DeepResearch,
        DEEP_RESEARCH_HARNESS_CAPABILITIES,
        "bitfun-core::agentic::agents::definitions::modes::deep_research",
    );
const MINIAPP_HARNESS_PROVIDER: HarnessProviderDescriptor =
    HarnessProviderDescriptor::legacy_facade(
        CORE_MINIAPP_HARNESS_PROVIDER_ID,
        HarnessWorkflow::MiniApp,
        MINIAPP_HARNESS_CAPABILITIES,
        "bitfun-core::miniapp",
    );

const NO_HARNESS_PROVIDERS: &[HarnessProviderDescriptor] = &[];
const DEEP_REVIEW_HARNESS_PROVIDERS: &[HarnessProviderDescriptor] = &[DEEP_REVIEW_HARNESS_PROVIDER];
const DEEP_RESEARCH_HARNESS_PROVIDERS: &[HarnessProviderDescriptor] =
    &[DEEP_RESEARCH_HARNESS_PROVIDER];
const MINIAPP_HARNESS_PROVIDERS: &[HarnessProviderDescriptor] = &[MINIAPP_HARNESS_PROVIDER];

const DEFAULT_PRODUCT_CAPABILITY_PACKS: &[ProductCapabilityPack] = &[
    ProductCapabilityPack::new(
        ProductCapabilityId::CodeAgent,
        CODE_AGENT_SERVICES,
        CODE_AGENT_TOOL_GROUPS,
        NO_HARNESS_PROVIDERS,
    ),
    ProductCapabilityPack::new(
        ProductCapabilityId::DeepReview,
        DEEP_REVIEW_SERVICES,
        INTEGRATION_TOOL_GROUPS,
        DEEP_REVIEW_HARNESS_PROVIDERS,
    ),
    ProductCapabilityPack::new(
        ProductCapabilityId::DeepResearch,
        DEEP_RESEARCH_SERVICES,
        INTEGRATION_TOOL_GROUPS,
        DEEP_RESEARCH_HARNESS_PROVIDERS,
    ),
    ProductCapabilityPack::new(
        ProductCapabilityId::MiniApp,
        MINIAPP_SERVICES,
        INTEGRATION_TOOL_GROUPS,
        MINIAPP_HARNESS_PROVIDERS,
    ),
];

pub fn default_product_capability_registry() -> ProductCapabilityRegistry {
    ProductCapabilityRegistry::new(DEFAULT_PRODUCT_CAPABILITY_PACKS)
}

pub fn default_product_tool_provider_group_plan() -> Vec<ToolProviderGroupPlan> {
    default_product_capability_registry().tool_provider_group_plan()
}

pub fn default_product_harness_registry() -> Result<HarnessRegistry, HarnessRegistryBuildError> {
    default_product_capability_registry().build_harness_registry()
}
