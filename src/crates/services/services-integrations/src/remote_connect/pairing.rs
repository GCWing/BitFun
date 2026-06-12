//! Pairing protocol for establishing E2E encrypted connections.
//!
//! Desktop generates a keypair + room, encodes it in a QR code.
//! Mobile scans QR, joins room, sends its public key.
//! Both sides derive a shared secret via ECDH and verify with a challenge-response.

use anyhow::{anyhow, Result};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::device::DeviceIdentity;
use super::encryption::{self, KeyPair};

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

/// Information encoded in the QR code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrPayload {
    pub url: String,
    pub room_id: String,
    pub device_id: String,
    pub device_name: String,
    pub public_key: String,
    pub version: u8,
}

/// Challenge sent from desktop to mobile during pairing verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingChallenge {
    pub challenge: String,
    pub timestamp: i64,
}

/// Response from mobile to desktop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingResponse {
    pub challenge_echo: String,
    pub device_id: String,
    pub device_name: String,
    #[serde(default)]
    pub mobile_install_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

/// Manages the pairing state machine.
pub struct PairingProtocol {
    state: Arc<RwLock<PairingState>>,
    keypair: Option<KeyPair>,
    shared_secret: Option<[u8; 32]>,
    room_id: Option<String>,
    device_identity: DeviceIdentity,
    challenge: Option<String>,
    peer_device_id: Option<String>,
    peer_device_name: Option<String>,
}

impl PairingProtocol {
    pub fn new(device_identity: DeviceIdentity) -> Self {
        Self {
            state: Arc::new(RwLock::new(PairingState::Idle)),
            keypair: None,
            shared_secret: None,
            room_id: None,
            device_identity,
            challenge: None,
            peer_device_id: None,
            peer_device_name: None,
        }
    }

    pub async fn state(&self) -> PairingState {
        self.state.read().await.clone()
    }

    pub fn shared_secret(&self) -> Option<&[u8; 32]> {
        self.shared_secret.as_ref()
    }

    pub fn room_id(&self) -> Option<&str> {
        self.room_id.as_deref()
    }

    pub fn peer_device_name(&self) -> Option<&str> {
        self.peer_device_name.as_deref()
    }

    /// Step 1 (Desktop): Generate keypair and prepare QR payload.
    pub async fn initiate(&mut self, relay_url: &str) -> Result<QrPayload> {
        let keypair = KeyPair::generate();
        let room_id = generate_room_id();

        let payload = QrPayload {
            url: relay_url.to_string(),
            room_id: room_id.clone(),
            device_id: self.device_identity.device_id.clone(),
            device_name: self.device_identity.device_name.clone(),
            public_key: keypair.public_key_base64(),
            version: 1,
        };

        self.keypair = Some(keypair);
        self.room_id = Some(room_id);
        *self.state.write().await = PairingState::WaitingForScan;

        Ok(payload)
    }

    /// Step 2 (Desktop): Peer joined with their public key — derive shared secret.
    pub async fn on_peer_joined(&mut self, peer_public_key_b64: &str) -> Result<PairingChallenge> {
        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| anyhow!("no keypair — call initiate() first"))?;

        let peer_pub = encryption::parse_public_key(peer_public_key_b64)?;
        let shared = keypair.derive_shared_secret(&peer_pub);
        self.shared_secret = Some(shared);

        let challenge = generate_challenge();
        self.challenge = Some(challenge.clone());

        let challenge_payload = PairingChallenge {
            challenge,
            timestamp: chrono::Utc::now().timestamp(),
        };

        *self.state.write().await = PairingState::Verifying;
        Ok(challenge_payload)
    }

    /// Step 3 (Desktop): Verify the peer's challenge response.
    pub async fn verify_response(&mut self, response: &PairingResponse) -> Result<bool> {
        let expected = self
            .challenge
            .as_ref()
            .ok_or_else(|| anyhow!("no challenge issued"))?;

        if response.challenge_echo != *expected {
            *self.state.write().await = PairingState::Failed {
                reason: "challenge mismatch".to_string(),
            };
            return Ok(false);
        }

        self.peer_device_id = Some(response.device_id.clone());
        self.peer_device_name = Some(response.device_name.clone());
        *self.state.write().await = PairingState::Connected;
        Ok(true)
    }

    /// Mobile side: process a received challenge and produce a response.
    pub fn answer_challenge(
        challenge: &PairingChallenge,
        device_identity: &DeviceIdentity,
        mobile_install_id: Option<String>,
        user_id: Option<String>,
    ) -> PairingResponse {
        PairingResponse {
            challenge_echo: challenge.challenge.clone(),
            device_id: device_identity.device_id.clone(),
            device_name: device_identity.device_name.clone(),
            mobile_install_id,
            user_id,
        }
    }

    pub async fn disconnect(&mut self) {
        *self.state.write().await = PairingState::Disconnected;
        self.shared_secret = None;
        self.challenge = None;
        self.peer_device_id = None;
        self.peer_device_name = None;
    }

    pub async fn reset(&mut self) {
        *self.state.write().await = PairingState::Idle;
        self.keypair = None;
        self.shared_secret = None;
        self.room_id = None;
        self.challenge = None;
        self.peer_device_id = None;
        self.peer_device_name = None;
    }

    pub async fn set_bot_connected(&mut self, peer_name: String) {
        self.peer_device_name = Some(peer_name);
        *self.state.write().await = PairingState::Connected;
    }

    /// Generate a 6-digit pairing code for bot connections.
    pub fn generate_bot_pairing_code() -> String {
        let code: u32 = rand::thread_rng().gen_range(100_000..1_000_000);
        format!("{code:06}")
    }
}

fn generate_room_id() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 8] = rng.gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn generate_challenge() -> String {
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pairing_flow() {
        let device = DeviceIdentity {
            device_id: "test-desktop-id".into(),
            device_name: "TestDesktop".into(),
            mac_address: "AA:BB:CC:DD:EE:FF".into(),
        };

        let mobile_device = DeviceIdentity {
            device_id: "test-mobile-id".into(),
            device_name: "TestMobile".into(),
            mac_address: "11:22:33:44:55:66".into(),
        };

        let mut protocol = PairingProtocol::new(device);

        // Step 1: Desktop initiates
        let qr = protocol.initiate("wss://relay.example.com").await.unwrap();
        assert_eq!(protocol.state().await, PairingState::WaitingForScan);
        assert!(!qr.room_id.is_empty());

        // Simulate mobile generating a keypair and joining
        let mobile_keypair = KeyPair::generate();
        let mobile_pub_b64 = mobile_keypair.public_key_base64();

        // Step 2: Desktop receives mobile's public key
        let challenge = protocol.on_peer_joined(&mobile_pub_b64).await.unwrap();
        assert_eq!(protocol.state().await, PairingState::Verifying);

        // Mobile answers the challenge
        let response = PairingProtocol::answer_challenge(
            &challenge,
            &mobile_device,
            Some("install-id-1".into()),
            Some("alice".into()),
        );

        // Step 3: Desktop verifies
        let ok = protocol.verify_response(&response).await.unwrap();
        assert!(ok);
        assert_eq!(protocol.state().await, PairingState::Connected);

        // Both sides should have matching shared secrets
        let desktop_secret = protocol.shared_secret().unwrap();
        let desktop_pub = encryption::parse_public_key(&qr.public_key).unwrap();
        let mobile_shared = mobile_keypair.derive_shared_secret(&desktop_pub);
        assert_eq!(*desktop_secret, mobile_shared);
    }

    #[test]
    fn test_bot_pairing_code() {
        let code = PairingProtocol::generate_bot_pairing_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }
}
