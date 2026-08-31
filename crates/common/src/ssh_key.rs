// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Inspection of SSH private key files.
//!
//! Both the CLI and the GTK GUI validate a key before a profile is saved: is it
//! encrypted, and if so does the passphrase work? That is a question about SSH
//! keys, not about either front-end, and it used to be answered by near-identical
//! copies in `ssh-tunnel-cli` and `ssh-tunnel-gui-gtk`. It lives here so there is
//! one implementation, and so only this crate needs to depend on `russh`.
//!
//! This is deliberately a *pre-flight* check on a file the user just chose. The
//! daemon does its own detection while actually loading a key for a connection,
//! using `russh::keys::Error::KeyIsEncrypted`; that path stays where it is.

use std::path::Path;

use russh::keys::decode_secret_key;

use crate::error::{Error, Result};

fn read_key(key_path: &Path) -> Result<String> {
    std::fs::read_to_string(key_path).map_err(|e| {
        Error::InvalidPath(format!(
            "Failed to read SSH key file {}: {}",
            key_path.display(),
            e
        ))
    })
}

/// Whether the key at `key_path` needs a passphrase.
///
/// A key that cannot be decoded without one is reported as encrypted. Note that
/// a *corrupt* key is indistinguishable here and is also reported as encrypted:
/// the caller then asks for a passphrase, and [`validate_key_passphrase`]
/// produces the real error. Anything more precise would mean second-guessing the
/// parser.
pub fn is_key_encrypted(key_path: &Path) -> Result<bool> {
    let key_data = read_key(key_path)?;
    Ok(decode_secret_key(&key_data, None).is_err())
}

/// Check that `passphrase` decrypts the key at `key_path`.
pub fn validate_key_passphrase(key_path: &Path, passphrase: &str) -> Result<()> {
    let key_data = read_key(key_path)?;
    decode_secret_key(&key_data, Some(passphrase))
        .map_err(|e| Error::Authentication(format!("Invalid passphrase or corrupted key: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    use russh::keys::ssh_key::{Algorithm, LineEnding, PrivateKey};

    const PASSPHRASE: &str = "correct-horse-battery-staple";

    /// Keys are generated per test rather than committed as fixtures.
    ///
    /// A private key file in the repository would be flagged by the `gitleaks`
    /// job -- correctly, since it cannot tell a throwaway from a real one -- and
    /// US-10.4's rule is that no credential-shaped file is committed at all.
    /// Generating them here keeps that rule absolute and costs nothing.
    fn generate(encrypted: bool) -> String {
        let key = PrivateKey::random(&mut rand::rng(), Algorithm::Ed25519).expect("generate key");
        let key = if encrypted {
            key.encrypt(&mut rand::rng(), PASSPHRASE)
                .expect("encrypt key")
        } else {
            key
        };
        key.to_openssh(LineEnding::LF)
            .expect("encode key")
            .to_string()
    }

    fn key_file(contents: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("id_ed25519");
        let mut f = std::fs::File::create(&path).expect("create key file");
        f.write_all(contents.as_bytes()).expect("write key");
        (dir, path)
    }

    #[test]
    fn an_unencrypted_key_is_not_reported_as_encrypted() {
        let (_dir, path) = key_file(&generate(false));
        assert!(!is_key_encrypted(&path).expect("should read the key"));
    }

    #[test]
    fn an_encrypted_key_is_reported_as_encrypted() {
        let (_dir, path) = key_file(&generate(true));
        assert!(is_key_encrypted(&path).expect("should read the key"));
    }

    #[test]
    fn the_correct_passphrase_validates() {
        let (_dir, path) = key_file(&generate(true));
        validate_key_passphrase(&path, PASSPHRASE)
            .expect("the correct passphrase should be accepted");
    }

    #[test]
    fn a_wrong_passphrase_is_rejected() {
        let (_dir, path) = key_file(&generate(true));
        let err = validate_key_passphrase(&path, "not-the-passphrase")
            .expect_err("a wrong passphrase must be rejected");
        assert!(
            err.to_string().contains("Invalid passphrase"),
            "the error should name the cause: {err}"
        );
    }

    /// Pins the documented ambiguity: malformed reads as encrypted.
    #[test]
    fn a_malformed_key_is_reported_as_encrypted() {
        let (_dir, path) = key_file("not an ssh key at all\n");
        assert!(is_key_encrypted(&path).expect("should read the file"));
    }

    #[test]
    fn a_missing_key_file_is_an_error_not_a_verdict() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(is_key_encrypted(&dir.path().join("nope")).is_err());
    }
}
