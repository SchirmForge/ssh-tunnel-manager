// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Client TLS Module
// Handles TLS certificate verification and pinning for HTTPS connections
// Shared between CLI and GUI clients

use std::sync::Arc;

use anyhow::Result;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, DigitallySignedStruct, Error as TlsError, RootCertStore, SignatureScheme,
};
use sha2::{Digest, Sha256};
use x509_parser::prelude::*;

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
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        // Calculate the fingerprint of the presented certificate
        let actual_fingerprint = Self::calculate_fingerprint(end_entity);

        // Compare with expected fingerprint
        if actual_fingerprint != self.expected_fingerprint {
            return Err(TlsError::InvalidCertificate(
                rustls::CertificateError::Other(rustls::OtherError(Arc::new(
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "Certificate fingerprint mismatch. Expected: {}, Got: {}",
                            self.expected_fingerprint, actual_fingerprint
                        ),
                    ),
                ))),
            ));
        }

        // Verify certificate is within its validity period
        // Even with pinning, we should reject expired certificates to avoid
        // accepting stolen/replayed certs with the same fingerprint
        let (_, cert) = X509Certificate::from_der(end_entity.as_ref()).map_err(|_| {
            TlsError::InvalidCertificate(rustls::CertificateError::BadEncoding)
        })?;

        // Check validity period using the provided timestamp
        let now_seconds = now.as_secs();
        let not_before = cert.validity().not_before.timestamp() as u64;
        let not_after = cert.validity().not_after.timestamp() as u64;

        if now_seconds < not_before {
            return Err(TlsError::InvalidCertificate(
                rustls::CertificateError::NotValidYet,
            ));
        }

        if now_seconds > not_after {
            return Err(TlsError::InvalidCertificate(
                rustls::CertificateError::Expired,
            ));
        }

        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        // Even with certificate pinning, we MUST verify the handshake signature
        // to ensure the server possesses the private key
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        // Even with certificate pinning, we MUST verify the handshake signature
        // to ensure the server possesses the private key
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::aws_lc_rs::default_provider().signature_verification_algorithms,
        )
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
