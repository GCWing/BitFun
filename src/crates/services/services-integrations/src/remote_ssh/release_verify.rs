//! Shared verification primitives for BitFun release assets.
//!
//! Remote installers must use the same build-time trust root as the CLI
//! self-updater. A SHA256 sidecar detects corruption, while the minisign
//! signature establishes who published that checksum or archive.

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};

/// Ed25519 (minisign) public key injected into official builds from the same
/// `TAURI_UPDATER_PUBKEY` used by the Desktop and CLI updaters.
///
/// Forks that publish their own releases set this at build time to their own
/// key. When absent, [`OFFICIAL_RELEASE_PUBKEY`] applies.
pub(crate) const RELEASE_PUBKEY: Option<&str> = option_env!("BITFUN_RELEASE_PUBKEY");

/// The official BitFun release public key (minisign key ID `50F47CBE6CC0A376`),
/// base64-wrapped the way Tauri wraps `minisign.pub`. This is public data —
/// every release publishes it as the `minisign.pub` asset — so compiling it in
/// lets development builds verify and install official releases instead of
/// failing closed with no trust root. Downloads still come only from the
/// pinned official repository, so trusting the matching official key here does
/// not widen what a build will execute.
pub(crate) const OFFICIAL_RELEASE_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDUwRjQ3Q0JFNkNDMEEzNzYKUldSMm84QnN2bnowVU9CYzNOb1RWVzA2d2RpR003cExQM0xwaUw0QTNTcDRueGtCc1dsSlJUeG4K";

pub(crate) fn release_pubkey() -> Option<&'static str> {
    RELEASE_PUBKEY
        .filter(|key| !key.trim().is_empty())
        .or(Some(OFFICIAL_RELEASE_PUBKEY))
}

pub(crate) fn require_release_pubkey() -> Result<&'static str> {
    release_pubkey().ok_or_else(|| {
        anyhow!("this build has no BitFun release signing key; refusing to install executable code")
    })
}

pub(crate) fn release_tag_for_version(version: &str) -> String {
    if version.contains("-nightly.") {
        "nightly".to_string()
    } else {
        format!("v{}", version.split('+').next().unwrap_or(version))
    }
}

/// Parse the first digest from a conventional `<hash>  <filename>` SHA256
/// sidecar. The filename is diagnostic only; release sidecars may use either
/// one or two spaces before it.
pub(crate) fn parse_sha256(checksum_text: &str, filename: &str) -> Result<String> {
    checksum_text
        .split_whitespace()
        .next()
        .filter(|value| value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| anyhow!("invalid SHA256 file for {filename}"))
}

pub(crate) fn verify_sha256(data: &[u8], expected: &str, filename: &str) -> Result<()> {
    let expected = expected.trim();
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!("invalid expected SHA256 for {filename}"));
    }
    let actual = format!("{:x}", Sha256::digest(data));
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(anyhow!("SHA256 mismatch for {filename}"));
    }
    Ok(())
}

/// Verify a Tauri-format `.sig` (base64 of a complete minisign signature file)
/// using the likewise base64-wrapped public key.
pub(crate) fn verify_minisign(data: &[u8], signature_b64: &str, pubkey_b64: &str) -> Result<()> {
    use base64::Engine as _;

    let decode = |value: &str, what: &str| -> Result<String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(value.trim().as_bytes())
            .with_context(|| format!("decode {what}"))?;
        String::from_utf8(bytes).with_context(|| format!("decode {what} as UTF-8"))
    };

    let public_key = minisign_verify::PublicKey::decode(&decode(pubkey_b64, "release public key")?)
        .map_err(|error| anyhow!("invalid release public key: {error}"))?;
    let signature =
        minisign_verify::Signature::decode(&decode(signature_b64, "release signature")?)
            .map_err(|error| anyhow!("invalid release signature: {error}"))?;
    public_key
        .verify(data, &signature, false)
        .map_err(|error| anyhow!("release signature does not match the signed data: {error}"))
}

/// Verify the minisign signature over a checksum sidecar, then return its
/// normalized digest. This is useful when the eventual target host has only a
/// SHA256 implementation and no copy of the BitFun trust root.
pub(crate) fn verify_signed_checksum(
    checksum_text: &str,
    signature_b64: &str,
    pubkey_b64: &str,
    filename: &str,
) -> Result<String> {
    verify_minisign(checksum_text.as_bytes(), signature_b64, pubkey_b64)?;
    parse_sha256(checksum_text, filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture produced with the real `minisign` CLI, then wrapped the way
    /// Tauri wraps keys and signatures.
    const FIXTURE_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXkgRTNFMDg3NENFQzFDMjJDMwpSV1RESWh6c1RJZmc0MXcyR3dpZWkwek5ES2FMWW05ZFFWcEVXTlEvVWxweXQybWJTMkpFMVUyTQo=";
    const FIXTURE_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIG1pbmlzaWduIHNlY3JldCBrZXkKUlVUREloenNUSWZnNDBMTitwb25aT3RCVy9VYmJtNWhkR1poM0lCb3IwUDBKaVZmZmM1cFJaNlZSNUpaSzNUUm1yWWpYMXFLQ2svWTdZUDhHdkRZT3YvanVoZlpnZmhyWEFRPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzg0OTUxOTM1CWZpbGU6YXJjaGl2ZS50YXIuZ3oJaGFzaGVkCjhWL21EUVAwZGdlZXVNU1lxWlpsOWdFSGUwOTJQTk9yRG1BMUV6ZHNQOUlEYkcyT1dneTFsQ1puUDBJaFIwQnJpMFBCeENRcUdDR2dpb0l0UGtSMUN3PT0K";
    const FIXTURE_DATA: &[u8] = b"hello-bitfun\n";

    #[test]
    fn embedded_trust_root_is_always_available_and_well_formed() {
        use base64::Engine as _;

        let pubkey = require_release_pubkey().expect("every build carries a trust root");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(pubkey.trim().as_bytes())
            .expect("trust root is base64");
        let text = String::from_utf8(decoded).expect("trust root decodes as UTF-8");
        minisign_verify::PublicKey::decode(&text).expect("trust root is a minisign public key");
    }

    #[test]
    fn minisign_wire_format_accepts_authentic_data_and_rejects_tampering() {
        verify_minisign(FIXTURE_DATA, FIXTURE_SIGNATURE, FIXTURE_PUBKEY)
            .expect("fixture signature must verify");
        assert!(verify_minisign(b"tampered\n", FIXTURE_SIGNATURE, FIXTURE_PUBKEY).is_err());
        assert!(verify_minisign(FIXTURE_DATA, "bm90LWEtc2ln", FIXTURE_PUBKEY).is_err());
    }

    #[test]
    fn sha256_sidecar_requires_a_well_formed_digest_and_matching_bytes() {
        let digest = format!("{:x}", Sha256::digest(FIXTURE_DATA));
        assert_eq!(
            parse_sha256(
                &format!("{digest}  bitfun-cli.tar.gz\n"),
                "bitfun-cli.tar.gz"
            )
            .unwrap(),
            digest
        );
        verify_sha256(FIXTURE_DATA, &digest, "bitfun-cli.tar.gz").unwrap();
        assert!(verify_sha256(FIXTURE_DATA, &"0".repeat(64), "bitfun-cli.tar.gz").is_err());
        assert!(parse_sha256("not-a-digest", "bitfun-cli.tar.gz").is_err());
    }

    #[test]
    fn release_tags_keep_stable_and_nightly_channels() {
        assert_eq!(release_tag_for_version("0.2.13"), "v0.2.13");
        assert_eq!(
            release_tag_for_version("0.2.14-nightly.20260724+abc123"),
            "nightly"
        );
    }
}
