//! Invitation status and account-linked IM enrollment codes.
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Current state of the pairing process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PairingState {
    Idle,
    WaitingForScan,
    Handshaking,
    Verifying,
    Connected,
    Failed { reason: String },
    Disconnected,
}

pub fn generate_bot_pairing_code() -> String {
    let code: u32 = rand::thread_rng().gen_range(100_000..1_000_000);
    format!("{code:06}")
}
