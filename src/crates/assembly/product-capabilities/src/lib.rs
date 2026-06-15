//! Product capability pack contracts.
//!
//! This crate owns provider-neutral product capability assembly facts. Concrete
//! workflow execution and tool implementations remain in their runtime owners.

use std::collections::HashSet;
use std::fmt;

use bitfun_harness::{
    build_descriptor_harness_registry, HarnessCapability, HarnessProviderDescriptor,
    HarnessRegistry, HarnessRegistryBuildError, HarnessWorkflow,
};
use bitfun_runtime_ports::RuntimeServiceCapability;
pub use bitfun_tool_packs::ToolProviderGroupPlanSelectionError as ProductCapabilityBuildError;
use bitfun_tool_packs::{try_product_tool_provider_group_plan_for_ids, ToolProviderGroupPlan};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DeliveryProfile {
    ProductFull,
    Desktop,
    Cli,
    Server,
    Remote,
    Acp,
    Web,
}

impl DeliveryProfile {
    pub const fn id(self) -> &'static str {
        match self {
            Self::ProductFull => "product-full",
            Self::Desktop => "desktop",
            Self::Cli => "cli",
            Self::Server => "server",
            Self::Remote => "remote",
            Self::Acp => "acp",
            Self::Web => "web",
        }
    }

    pub const fn all_current_product_profiles() -> &'static [DeliveryProfile] {
        &[
            Self::ProductFull,
            Self::Desktop,
            Self::Cli,
            Self::Server,
            Self::Remote,
            Self::Acp,
            Self::Web,
        ]
    }
}

impl fmt::Display for DeliveryProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProductServiceCapabilityRequirement {
    capability_id: ProductCapabilityId,
    service_capability: RuntimeServiceCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProductServiceCapabilityStatus {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProductServiceCapabilityAvailability {
    requirement: ProductServiceCapabilityRequirement,
    status: ProductServiceCapabilityStatus,
}

impl ProductServiceCapabilityAvailability {
    pub const fn new(
        requirement: ProductServiceCapabilityRequirement,
        status: ProductServiceCapabilityStatus,
    ) -> Self {
        Self {
            requirement,
            status,
        }
    }

    pub const fn requirement(self) -> ProductServiceCapabilityRequirement {
        self.requirement
    }

    pub const fn status(self) -> ProductServiceCapabilityStatus {
        self.status
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductCapabilitySet {
    ids: Vec<ProductCapabilityId>,
}

impl ProductCapabilitySet {
    pub fn new(ids: Vec<ProductCapabilityId>) -> Self {
        let mut deduped = Vec::new();
        for id in ids {
            if !deduped.contains(&id) {
                deduped.push(id);
            }
        }
        Self { ids: deduped }
    }

    pub fn ids(&self) -> &[ProductCapabilityId] {
        &self.ids
    }

    pub fn contains(&self, id: ProductCapabilityId) -> bool {
        self.ids.contains(&id)
    }
}

impl ProductServiceCapabilityRequirement {
    pub const fn new(
        capability_id: ProductCapabilityId,
        service_capability: RuntimeServiceCapability,
    ) -> Self {
        Self {
            capability_id,
            service_capability,
        }
    }

    pub const fn capability_id(self) -> ProductCapabilityId {
        self.capability_id
    }

    pub const fn service_capability(self) -> RuntimeServiceCapability {
        self.service_capability
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductCapabilityAssembly {
    capability_ids: Vec<ProductCapabilityId>,
    service_requirements: Vec<ProductServiceCapabilityRequirement>,
    tool_provider_group_plan: Vec<ToolProviderGroupPlan>,
    harness_provider_descriptors: Vec<HarnessProviderDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductAssemblyPlan {
    profile: DeliveryProfile,
    capability_set: ProductCapabilitySet,
    capability_assembly: ProductCapabilityAssembly,
}

impl ProductAssemblyPlan {
    pub fn new(
        profile: DeliveryProfile,
        capability_set: ProductCapabilitySet,
        capability_assembly: ProductCapabilityAssembly,
    ) -> Self {
        Self {
            profile,
            capability_set,
            capability_assembly,
        }
    }

    pub const fn profile(&self) -> DeliveryProfile {
        self.profile
    }

    pub fn capability_set(&self) -> &ProductCapabilitySet {
        &self.capability_set
    }

    pub fn capability_assembly(&self) -> &ProductCapabilityAssembly {
        &self.capability_assembly
    }

    pub fn service_availability_report<F>(
        &self,
        mut is_available: F,
    ) -> Vec<ProductServiceCapabilityAvailability>
    where
        F: FnMut(RuntimeServiceCapability) -> bool,
    {
        self.capability_assembly
            .service_requirements()
            .iter()
            .copied()
            .map(|requirement| {
                let status = if is_available(requirement.service_capability()) {
                    ProductServiceCapabilityStatus::Available
                } else {
                    ProductServiceCapabilityStatus::Unavailable
                };
                ProductServiceCapabilityAvailability::new(requirement, status)
            })
            .collect()
    }

    pub fn build_harness_registry(&self) -> Result<HarnessRegistry, HarnessRegistryBuildError> {
        self.capability_assembly.build_harness_registry()
    }
}

impl ProductCapabilityAssembly {
    fn new(
        capability_ids: Vec<ProductCapabilityId>,
        service_requirements: Vec<ProductServiceCapabilityRequirement>,
        tool_provider_group_plan: Vec<ToolProviderGroupPlan>,
        harness_provider_descriptors: Vec<HarnessProviderDescriptor>,
    ) -> Self {
        Self {
            capability_ids,
            service_requirements,
            tool_provider_group_plan,
            harness_provider_descriptors,
        }
    }

    pub fn capability_ids(&self) -> &[ProductCapabilityId] {
        &self.capability_ids
    }

    pub fn service_requirements(&self) -> &[ProductServiceCapabilityRequirement] {
        &self.service_requirements
    }

    pub fn required_service_capabilities(&self) -> Vec<RuntimeServiceCapability> {
        let mut seen = HashSet::new();
        let mut capabilities = Vec::new();
        for requirement in &self.service_requirements {
            let capability = requirement.service_capability();
            if seen.insert(capability) {
                capabilities.push(capability);
            }
        }
        capabilities
    }

    pub fn missing_service_requirements<F>(
        &self,
        mut is_available: F,
    ) -> Vec<ProductServiceCapabilityRequirement>
    where
        F: FnMut(RuntimeServiceCapability) -> bool,
    {
        self.service_requirements
            .iter()
            .copied()
            .filter(|requirement| !is_available(requirement.service_capability()))
            .collect()
    }

    pub fn tool_provider_group_plan(&self) -> &[ToolProviderGroupPlan] {
        &self.tool_provider_group_plan
    }

    pub fn harness_provider_descriptors(&self) -> &[HarnessProviderDescriptor] {
        &self.harness_provider_descriptors
    }

    pub fn build_harness_registry(&self) -> Result<HarnessRegistry, HarnessRegistryBuildError> {
        build_descriptor_harness_registry(self.harness_provider_descriptors.iter().copied())
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
        self.service_requirements()
            .into_iter()
            .map(|requirement| requirement.service_capability())
            .fold(Vec::new(), |mut capabilities, capability| {
                if !capabilities.contains(&capability) {
                    capabilities.push(capability);
                }
                capabilities
            })
    }

    pub fn service_requirements(self) -> Vec<ProductServiceCapabilityRequirement> {
        let mut seen = HashSet::new();
        let mut requirements = Vec::new();
        for pack in self.packs {
            for service_capability in pack.required_services() {
                let requirement =
                    ProductServiceCapabilityRequirement::new(pack.id(), *service_capability);
                if seen.insert(requirement) {
                    requirements.push(requirement);
                }
            }
        }
        requirements
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
        try_product_tool_provider_group_plan_for_ids(&provider_ids)
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
        build_descriptor_harness_registry(self.harness_provider_descriptors())
    }

    pub fn try_build_assembly(
        self,
    ) -> Result<ProductCapabilityAssembly, ProductCapabilityBuildError> {
        Ok(ProductCapabilityAssembly::new(
            self.capability_ids(),
            self.service_requirements(),
            self.try_tool_provider_group_plan()?,
            self.harness_provider_descriptors(),
        ))
    }

    pub fn build_assembly(self) -> ProductCapabilityAssembly {
        self.try_build_assembly()
            .expect("product capability packs must build a valid assembly")
    }

    pub fn capability_set(self) -> ProductCapabilitySet {
        ProductCapabilitySet::new(self.capability_ids())
    }

    pub fn build_assembly_plan(self, profile: DeliveryProfile) -> ProductAssemblyPlan {
        let capability_set = self.capability_set();
        let capability_assembly = self.build_assembly();
        ProductAssemblyPlan::new(profile, capability_set, capability_assembly)
    }
}

const CODE_AGENT_SERVICES: &[RuntimeServiceCapability] = &[
    RuntimeServiceCapability::FileSystem,
    RuntimeServiceCapability::Workspace,
    RuntimeServiceCapability::SessionStore,
    RuntimeServiceCapability::Permission,
    RuntimeServiceCapability::Events,
    RuntimeServiceCapability::Clock,
    RuntimeServiceCapability::Terminal,
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

pub fn default_product_capability_assembly() -> ProductCapabilityAssembly {
    default_product_capability_registry().build_assembly()
}

pub fn product_assembly_plan_for_profile(profile: DeliveryProfile) -> ProductAssemblyPlan {
    default_product_capability_registry().build_assembly_plan(profile)
}

pub fn default_product_assembly_plan() -> ProductAssemblyPlan {
    product_assembly_plan_for_profile(DeliveryProfile::ProductFull)
}

pub fn product_harness_registry_for_profile(
    profile: DeliveryProfile,
) -> Result<HarnessRegistry, HarnessRegistryBuildError> {
    product_assembly_plan_for_profile(profile).build_harness_registry()
}

pub fn default_product_harness_registry() -> Result<HarnessRegistry, HarnessRegistryBuildError> {
    product_harness_registry_for_profile(DeliveryProfile::ProductFull)
}
