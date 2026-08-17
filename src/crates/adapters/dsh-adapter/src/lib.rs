//! DeepSeek Harness (`dsh`) bundle source adapter.
//!
//! The production surface is intentionally small: load a `dsh`-compatible
//! managed package (its `package.json` bundle/profile declaration and any
//! declared `cordis.patch.yml` bundle patch) as a projection-only plugin runtime
//! adapter. The adapter does not execute Cordis plugins, install npm packages,
//! or depend on a user-local `dsh` CLI.

mod source_adapter;

pub use source_adapter::load_dsh_package_adapter;
