//! Shared verification primitives for OpenBitFun release assets.
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
pub(crate) const RELEASE_PUBKEY: Option<&str> = option_env!("OPENBITFUN_RELEASE_PUBKEY");

/// The official OpenBitFun release public key (minisign key ID `50F47CBE6CC0A376`),
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
        anyhow!(
            "this build has no OpenBitFun release signing key; refusing to install executable code"
        )
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

/// Verify a Tauri-format `.sig` (base64 of a complete minisign signature file).
/// The public key may be the raw minisign file accepted by current Tauri CLIs
/// or the legacy base64 wrapper stored by older release configurations.
pub(crate) fn verify_minisign(data: &[u8], signature_b64: &str, pubkey: &str) -> Result<()> {
    use base64::Engine as _;

    let public_key = minisign_verify::PublicKey::decode(&decode_public_key(pubkey)?)
        .map_err(|error| anyhow!("invalid release public key: {error}"))?;
    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim().as_bytes())
        .context("decode release signature")?;
    let signature_text =
        String::from_utf8(signature_bytes).context("decode release signature as UTF-8")?;
    let signature = minisign_verify::Signature::decode(&signature_text)
        .map_err(|error| anyhow!("invalid release signature: {error}"))?;
    public_key
        .verify(data, &signature, false)
        .map_err(|error| anyhow!("release signature does not match the signed data: {error}"))
}

fn decode_public_key(value: &str) -> Result<String> {
    use base64::Engine as _;

    let value = value.trim();
    if value.starts_with("untrusted comment:") {
        return Ok(value.to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.as_bytes())
        .context("decode release public key")?;
    String::from_utf8(bytes).context("decode release public key as UTF-8")
}

/// Verify the minisign signature over a checksum sidecar, then return its
/// normalized digest. This is useful when the eventual target host has only a
/// SHA256 implementation and no copy of the OpenBitFun trust root.
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

    /// Fixture produced with the Tauri signer CLI in its base64-wrapped
    /// minisign wire format.
    const FIXTURE_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IERENTQzQUM5RUY0NTIzRTMKUldUakkwWHZ5VHBVM1NOMXJWMHhLVlljSDBOY2x4YlpxVHA2clN1NEJPMWcyY2Qvd2U4VUR2b3AK";
    const FIXTURE_SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVUakkwWHZ5VHBVM2RVVFdoR3FNZDltSWNUeEQ1K2ZnNWRUSnYxWk5lUkZzd0h0MkdzSUhUSlV6a0haUTdNZm1aemM5QVBQWW50UWgvaWpFcEp1Zkp4SERWdnhIc1g2YUFrPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzg4NDg2NTU4CWZpbGU6Lm9wZW5iaXRmdW4tbWluaXNpZ24tZml4dHVyZS50eHQKa1QxdDQ3bWtLVlhaZUdFSjR4R0V5R1Z3REVnUlI0RGJqbHFoZkVHdkdLSlFyTGJ5Z05JRTI5V3dwdXRkSFpZckUrK0RaUVVJYUJod1dzcmVydHZnQXc9PQo=";
    const FIXTURE_DATA: &[u8] = b"hello-openbitfun\n";

    #[test]
    fn embedded_trust_root_is_always_available_and_well_formed() {
        let pubkey = require_release_pubkey().expect("every build carries a trust root");
        let text = decode_public_key(pubkey).expect("trust root has a supported key format");
        minisign_verify::PublicKey::decode(&text).expect("trust root is a minisign public key");
    }

    #[test]
    fn minisign_wire_format_accepts_authentic_data_and_rejects_tampering() {
        verify_minisign(FIXTURE_DATA, FIXTURE_SIGNATURE, FIXTURE_PUBKEY)
            .expect("fixture signature must verify");
        let raw_pubkey = decode_public_key(FIXTURE_PUBKEY).expect("decode fixture public key");
        verify_minisign(FIXTURE_DATA, FIXTURE_SIGNATURE, &raw_pubkey)
            .expect("raw minisign public key must verify");
        assert!(verify_minisign(b"tampered\n", FIXTURE_SIGNATURE, FIXTURE_PUBKEY).is_err());
        assert!(verify_minisign(FIXTURE_DATA, "bm90LWEtc2ln", FIXTURE_PUBKEY).is_err());
    }

    #[test]
    fn sha256_sidecar_requires_a_well_formed_digest_and_matching_bytes() {
        let digest = format!("{:x}", Sha256::digest(FIXTURE_DATA));
        assert_eq!(
            parse_sha256(
                &format!("{digest}  openbitfun-cli.tar.gz\n"),
                "openbitfun-cli.tar.gz"
            )
            .unwrap(),
            digest
        );
        verify_sha256(FIXTURE_DATA, &digest, "openbitfun-cli.tar.gz").unwrap();
        assert!(verify_sha256(FIXTURE_DATA, &"0".repeat(64), "openbitfun-cli.tar.gz").is_err());
        assert!(parse_sha256("not-a-digest", "openbitfun-cli.tar.gz").is_err());
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
