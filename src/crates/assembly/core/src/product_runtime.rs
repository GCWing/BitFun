//! Core product-full runtime adapter boundary.
//!
//! Product runtime assembly facts live in `bitfun-product-capabilities`. Core
//! keeps only compatibility exports and adapter wiring that still depends on
//! existing concrete core paths.

mod runtime_services;

pub use bitfun_product_capabilities::ProductRuntimeAssembly as CoreProductRuntimeAssembly;
pub use runtime_services::CoreRuntimeServicesProvider;
