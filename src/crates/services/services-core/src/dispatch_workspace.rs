//! Transport-neutral helpers shared by detached Dispatch adapters.
//!
//! Dispatch protocol v3 transfers Git bundles. The former file-snapshot
//! archive, manifest, conflict, and path-overwrite APIs intentionally no
//! longer live here: without a common Git commit they could only provide an
//! unsafe fallback that bypassed the worktree baseline contract.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Digest of an in-memory Git bundle or Relay chunk assembly.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Stream a file into SHA-256 without loading a potentially large bundle into
/// memory.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("hash {}", path.display()))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_and_file_digests_match() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("objects.bundle");
        let bytes = b"Git bundle bytes";
        std::fs::write(&path, bytes).expect("write bundle");

        assert_eq!(
            sha256_file(&path).expect("file digest"),
            sha256_bytes(bytes)
        );
    }
}
