//! taiji-pattern — Chart pattern recognition via multi-dimensional DTW.
//!
//! # Modules
//!
//! - [`dtw`] — DtwEngine: weighted Euclidean DTW + LB_Keogh lower bound
//! - [`index`] — PatternIndex: three-layer index (signature → LB_Keogh → DTW)
//! - [`node`] — PatternMatchNode: ComputeNode that feeds bars into the index

pub mod dtw;
pub mod index;
pub mod node;

pub use dtw::DtwEngine;
pub use index::{PatternIndex, PatternMatch};
pub use node::PatternMatchNode;
