//! Product assembly compatibility facade.
//!
//! Provider-neutral product assembly facts are owned by
//! `bitfun-product-capabilities`. Core-specific runtime service adapters live
//! under `product_runtime`.

pub use bitfun_product_capabilities::{
    default_product_assembly_plan, default_product_capability_assembly,
    default_product_capability_registry, default_product_harness_registry,
    product_assembly_plan_for_profile, DeliveryProfile, ProductAssemblyPlan,
    ProductCapabilityAssembly, ProductCapabilityId, ProductCapabilityPack,
    ProductCapabilityRegistry, ProductCapabilitySet, ProductServiceCapabilityAvailability,
    ProductServiceCapabilityRequirement, ProductServiceCapabilityStatus,
};

pub use crate::product_runtime::CoreRuntimeServicesProvider;
