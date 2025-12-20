// SSH Tunnel Manager - Client TLS Module
// Handles TLS certificate verification and pinning for HTTPS connections
// Shared between CLI and GUI clients

use std::sync::Arc;

use anyhow::Result;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, Error as TlsError, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};

/// Custom certificate verifier that pins to a specific certificate fingerprint
#[derive(Debug)]
struct FingerprintVerifier {
    expected_fingerprint: String,
}

impl FingerprintVerifier {
    fn new(fingerprint: String) -> Self {
        Self {
            expected_fingerprint: fingerprint,
        }
    }

    /// Calculate SHA256 fingerprint of a certificate
    fn calculate_fingerprint(cert: &CertificateDer) -> String {
        let mut hasher = Sha256::new();
        hasher.update(cert.as_ref());
        let result = hasher.finalize();

        // Format as colon-separated hex (e.g., "AA:BB:CC:...")
        result
            .iter()
            .map(|b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(":")
    }
}

impl ServerCertVerifier for FingerprintVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        // Calculate the fingerprint of the presented certificate
        let actual_fingerprint = Self::calculate_fingerprint(end_entity);

        // Compare with expected fingerprint
        if actual_fingerprint == self.expected_fingerprint {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(TlsError::InvalidCertificate(
                rustls::CertificateError::Other(rustls::OtherError(Arc::new(
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "Certificate fingerprint mismatch. Expected: {}, Got: {}",
                            self.expected_fingerprint, actual_fingerprint
                        ),
                    ),
                ))),
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        // Accept any signature for pinned certificates
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        // Accept any signature for pinned certificates
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        // Support all signature schemes
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::ECDSA_NISTP521_SHA512,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

/// Create a rustls ClientConfig with certificate pinning
pub fn create_pinned_tls_config(fingerprint: String) -> Result<ClientConfig> {
    // Install default crypto provider if not already set
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let verifier = FingerprintVerifier::new(fingerprint);

    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();

    Ok(config)
}

/// Create a rustls ClientConfig that accepts any certificate (for HTTP mode or no pinning)
pub fn create_insecure_tls_config() -> Result<ClientConfig> {
    // Install default crypto provider if not already set
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Use default verification with system/webpki roots
    let mut root_store = RootCertStore::empty();

    // Add webpki roots
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint_calculation() {
        // Create a dummy certificate (this is just for testing the fingerprint format)
        let dummy_cert = CertificateDer::from(vec![1, 2, 3, 4, 5]);
        let fingerprint = FingerprintVerifier::calculate_fingerprint(&dummy_cert);

        // Should be colon-separated hex
        assert!(fingerprint.contains(':'));
        assert!(fingerprint
            .split(':')
            .all(|s| s.len() == 2 && s.chars().all(|c| c.is_ascii_hexdigit())));
    }
}
