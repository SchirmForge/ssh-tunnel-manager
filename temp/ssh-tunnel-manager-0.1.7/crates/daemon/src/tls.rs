// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - TLS Module
// Handles TLS certificate generation and loading

use std::fs;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use tracing::{info, warn};

/// Generate a self-signed certificate for the daemon
/// Returns the certificate fingerprint
pub fn generate_self_signed_cert(
    cert_path: &Path,
    key_path: &Path,
) -> Result<String> {
    info!("Generating self-signed TLS certificate");

    // Create distinguished name
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "SSH Tunnel Manager Daemon");
    dn.push(DnType::OrganizationName, "SSH Tunnel Manager");

    // Create certificate parameters
    let mut params = CertificateParams::new(vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "::1".to_string(),
    ])?;
    params.distinguished_name = dn;
    // Valid from 1 day ago (to handle clock skew) to 10 years in the future
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    params.not_after = time::OffsetDateTime::now_utc() + time::Duration::days(3650);

    // Generate certificate
    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;

    // Ensure parent directories exist
    if let Some(parent) = cert_path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create certificate directory")?;
    }

    // Write certificate (PEM format)
    fs::write(cert_path, cert.pem())
        .context("Failed to write certificate file")?;

    // Write private key (PEM format)
    fs::write(key_path, key_pair.serialize_pem())
        .context("Failed to write private key file")?;

    // Set restrictive permissions on both files (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(cert_path, permissions.clone())
            .context("Failed to set certificate permissions")?;
        fs::set_permissions(key_path, permissions)
            .context("Failed to set private key permissions")?;
    }

    // Calculate and display certificate fingerprint
    let fingerprint = calculate_fingerprint(&cert.der());
    info!("Certificate generated successfully");
    info!("Certificate: {}", cert_path.display());
    info!("Private key: {}", key_path.display());
    info!("Certificate fingerprint (SHA256): {}", fingerprint);
    info!("");
    info!("⚠️  IMPORTANT: Clients will need to trust this certificate!");
    info!("   Use this fingerprint to verify the certificate on first connect.");

    Ok(fingerprint)
}

/// Calculate SHA256 fingerprint of a certificate
fn calculate_fingerprint(der: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(der);
    let result = hasher.finalize();

    // Format as colon-separated hex
    result
        .iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<_>>()
        .join(":")
}

/// Calculate fingerprint from an existing certificate file
pub fn get_cert_fingerprint(cert_path: &Path) -> Result<String> {
    let cert_file = fs::File::open(cert_path)
        .context("Failed to open certificate file")?;
    let mut cert_reader = std::io::BufReader::new(cert_file);

    let certs = certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificate")?;

    if certs.is_empty() {
        anyhow::bail!("No certificates found in file");
    }

    Ok(calculate_fingerprint(&certs[0]))
}

/// Check certificate expiry and warn if approaching expiration
/// Returns true if certificate needs regeneration (expires within 30 days or already expired)
pub fn check_cert_expiry(cert_path: &Path) -> Result<bool> {
    use x509_parser::prelude::*;

    // Read and parse certificate
    let cert_file = fs::File::open(cert_path)
        .context("Failed to open certificate file")?;
    let mut cert_reader = std::io::BufReader::new(cert_file);

    let cert_ders = certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificate")?;

    if cert_ders.is_empty() {
        anyhow::bail!("No certificates found in file");
    }

    let (_, cert) = X509Certificate::from_der(&cert_ders[0])
        .map_err(|e| anyhow::anyhow!("Failed to parse certificate DER: {}", e))?;

    let now = ::time::OffsetDateTime::now_utc().unix_timestamp();
    let not_after = cert.validity().not_after.timestamp();
    let days_until_expiry = (not_after - now) / 86400; // seconds to days

    if days_until_expiry <= 0 {
        warn!("═══════════════════════════════════════════════════════════");
        warn!("⚠️  TLS CERTIFICATE EXPIRED!");
        warn!("═══════════════════════════════════════════════════════════");
        warn!("The TLS certificate has expired.");
        warn!("Certificate will be regenerated automatically.");
        warn!("Clients will need to update their pinned fingerprint.");
        warn!("═══════════════════════════════════════════════════════════");
        return Ok(true);
    } else if days_until_expiry <= 30 {
        warn!("═══════════════════════════════════════════════════════════");
        warn!("⚠️  TLS CERTIFICATE EXPIRING SOON!");
        warn!("═══════════════════════════════════════════════════════════");
        warn!("The TLS certificate will expire in {} days.", days_until_expiry);
        warn!("Consider regenerating the certificate soon.");
        warn!("Delete {} to regenerate on next start.", cert_path.display());
        warn!("═══════════════════════════════════════════════════════════");
        return Ok(true);
    } else if days_until_expiry <= 90 {
        info!("TLS certificate will expire in {} days", days_until_expiry);
    }

    Ok(false)
}

/// Load TLS certificate and private key from files
pub fn load_tls_cert_and_key(
    cert_path: &Path,
    key_path: &Path,
) -> Result<(Vec<CertificateDer<'static>>, PrivateKeyDer<'static>)> {
    info!("Loading TLS certificate from: {}", cert_path.display());
    info!("Loading TLS private key from: {}", key_path.display());

    // Read certificate file
    let cert_file = fs::File::open(cert_path)
        .context("Failed to open certificate file")?;
    let mut cert_reader = std::io::BufReader::new(cert_file);

    let certs = certs(&mut cert_reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificate file")?;

    if certs.is_empty() {
        anyhow::bail!("No certificates found in certificate file");
    }

    // Read private key file
    let key_file = fs::File::open(key_path)
        .context("Failed to open private key file")?;
    let mut key_reader = std::io::BufReader::new(key_file);

    let key = private_key(&mut key_reader)
        .context("Failed to parse private key file")?
        .ok_or_else(|| anyhow::anyhow!("No private key found in private key file"))?;

    info!("TLS certificate and key loaded successfully");

    Ok((certs, key))
}

/// Create a rustls ServerConfig from certificate and key files
pub fn create_tls_config(cert_path: &Path, key_path: &Path) -> Result<Arc<ServerConfig>> {
    let mut needs_regeneration = false;

    // Check if certificate/key exist
    if !cert_path.exists() || !key_path.exists() {
        if cert_path.exists() {
            warn!("Certificate exists but private key is missing, regenerating both");
        } else if key_path.exists() {
            warn!("Private key exists but certificate is missing, regenerating both");
        }
        needs_regeneration = true;
    } else {
        // Check if certificate is expired or expiring soon (within 30 days)
        match check_cert_expiry(cert_path) {
            Ok(should_regenerate) => {
                if should_regenerate {
                    needs_regeneration = true;
                }
            }
            Err(e) => {
                warn!("Failed to check certificate expiry: {}", e);
                warn!("Regenerating certificate to be safe");
                needs_regeneration = true;
            }
        }
    }

    // Regenerate if needed
    if needs_regeneration {
        generate_self_signed_cert(cert_path, key_path)?;
    }

    // Load certificate and key
    let (certs, key) = load_tls_cert_and_key(cert_path, key_path)?;

    // Install default crypto provider if not already set
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Create rustls server config
    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to create TLS configuration")?;

    // Enable HTTP/2 and HTTP/1.1
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_and_load_certificate() {
        let temp_dir = TempDir::new().unwrap();
        let cert_path = temp_dir.path().join("test.crt");
        let key_path = temp_dir.path().join("test.key");

        // Generate certificate
        generate_self_signed_cert(&cert_path, &key_path).unwrap();

        // Verify files were created
        assert!(cert_path.exists());
        assert!(key_path.exists());

        // Verify we can load them
        let (certs, _key) = load_tls_cert_and_key(&cert_path, &key_path).unwrap();
        assert!(!certs.is_empty());

        // Verify we can create TLS config
        let _config = create_tls_config(&cert_path, &key_path).unwrap();
    }

    #[test]
    fn test_auto_generate_on_missing_cert() {
        let temp_dir = TempDir::new().unwrap();
        let cert_path = temp_dir.path().join("auto.crt");
        let key_path = temp_dir.path().join("auto.key");

        // Should auto-generate if missing
        let _config = create_tls_config(&cert_path, &key_path).unwrap();

        // Verify files were created
        assert!(cert_path.exists());
        assert!(key_path.exists());
    }
}
