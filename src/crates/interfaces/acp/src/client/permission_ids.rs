//! Shared ACP permission id helpers.
//!
//! Keep the minted prefix and remote detectors in one place so Desktop remote
//! control and the ACP client cannot drift apart.

pub const ACP_PERMISSION_ID_PREFIX: &str = "acp_permission_";

pub fn is_acp_permission_id(permission_id: &str) -> bool {
    permission_id.starts_with(ACP_PERMISSION_ID_PREFIX)
}

pub fn new_acp_permission_id() -> String {
    format!("{ACP_PERMISSION_ID_PREFIX}{}", uuid::Uuid::new_v4())
}
