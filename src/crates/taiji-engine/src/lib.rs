//! Taiji trading engine — DAG-based compute pipeline.
//! Architecture: tick → BarGenerator → DAG (ComputeNode graph) → signals.

pub mod compliance;
pub mod config;
pub mod dag;
pub mod debate;
pub mod error;
pub mod factory;
pub mod feature_flags;
pub mod fusion;
pub mod node;
pub mod pipeline;
pub mod risk;
pub mod signal;
pub mod source;
pub mod state;
pub mod store;
pub mod types;
