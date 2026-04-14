#![allow(clippy::expect_used)]

mod common;

#[path = "e2e/search_index.rs"]
mod search_index;

#[path = "e2e/search_engine.rs"]
mod search_engine;

#[path = "e2e/filtering_api.rs"]
mod filtering_api;

#[path = "e2e/workspace_index.rs"]
mod workspace_index;
