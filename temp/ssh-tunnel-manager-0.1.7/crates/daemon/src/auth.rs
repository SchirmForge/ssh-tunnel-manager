// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

// SSH Tunnel Manager - Authentication Module
// Handles token-based authentication for the daemon API

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use tracing::{info, warn};
use uuid::Uuid;
use zeroize::Zeroizing;

/// HTTP header name for authentication token
pub const AUTH_TOKEN_HEADER: &str = "X-Tunnel-Token";

/// Obfuscate a token for logging (show only last 4 characters)
/// Example: "abc123def456" -> "********f456"
pub fn obfuscate_token(token: &str) -> String {
    if token.len() < 4 {
        // If token is very short, just mask everything
        "*".repeat(token.len())
    } else {
        let visible_chars = 4;
        let mask_len = token.len() - visible_chars;
        format!("{}{}", "*".repeat(mask_len), &token[mask_len..])
    }
}

/// Generate a new authentication token
pub fn generate_token() -> String {
    Uuid::new_v4().to_string()
}

/// Load or generate authentication token from file
/// Returns (token, was_newly_generated)
pub fn load_or_generate_token(token_path: &PathBuf) -> Result<(String, bool)> {
    // If token file exists, load it
    if token_path.exists() {
        let token = fs::read_to_string(token_path)
            .context("Failed to read authentication token file")?
            .trim()
            .to_string();

        if token.is_empty() {
            warn!("Token file exists but is empty, regenerating");
        } else {
            info!("Loaded authentication token from: {}", token_path.display());
            return Ok((token, false));
        }
    }

    // Generate new token
    let token = generate_token();
    save_token(token_path, &token)?;

    info!("Generated new authentication token");
    info!("Token saved to: {}", token_path.display());
    info!("");
    info!("⚠️  IMPORTANT: Clients must provide this token to connect!");
    info!("   Token: {} (full token in {})", obfuscate_token(&token), token_path.display());
    info!("   Add to CLI config or use X-Tunnel-Token header");

    Ok((token, true))
}

/// Save authentication token to file
fn save_token(token_path: &PathBuf, token: &str) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = token_path.parent() {
        fs::create_dir_all(parent).context("Failed to create token directory")?;
    }

    // Write token to file
    fs::write(token_path, token).context("Failed to write token file")?;

    // Set restrictive permissions (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        fs::set_permissions(token_path, permissions)
            .context("Failed to set token file permissions")?;
    }

    Ok(())
}

/// Authentication middleware state
#[derive(Clone)]
pub struct AuthState {
    token: Zeroizing<String>,
}

impl AuthState {
    pub fn new(token: String) -> Self {
        Self {
            token: Zeroizing::new(token),
        }
    }
}

/// Authentication middleware for Axum
///
/// This middleware checks for the X-Tunnel-Token header and validates it
/// against the configured token. Returns 401 Unauthorized if token is missing
/// or invalid.
pub async fn auth_middleware(
    axum::extract::State(auth_state): axum::extract::State<AuthState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get the provided token from headers
    let provided_token = request
        .headers()
        .get(AUTH_TOKEN_HEADER)
        .and_then(|h| h.to_str().ok());

    // Validate token
    match provided_token {
        Some(token) if token == auth_state.token.as_str() => {
            // Too chatty at debug when clients poll frequently; keep at trace.
            tracing::trace!("Authentication successful");
            Ok(next.run(request).await)
        }
        Some(_) => {
            warn!("Authentication failed: invalid token");
            Err(StatusCode::UNAUTHORIZED)
        }
        None => {
            warn!("Authentication failed: missing token");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_obfuscate_token() {
        // Normal token (UUID format is 36 chars)
        let token = "abc123de-f456-7890-1234-567890abcdef";
        let obfuscated = obfuscate_token(token);
        assert_eq!(obfuscated, "********************************cdef");
        assert_eq!(obfuscated.len(), token.len());

        // Short token (4 chars)
        let short = "1234";
        assert_eq!(obfuscate_token(short), "1234");

        // Very short token (less than 4 chars)
        let very_short = "abc";
        assert_eq!(obfuscate_token(very_short), "***");

        // Longer custom token
        let custom = "my-secret-token-12345";
        let obfuscated_custom = obfuscate_token(custom);
        assert_eq!(obfuscated_custom, "*****************2345");
        assert!(obfuscated_custom.ends_with("2345"));
    }

    #[test]
    fn test_generate_token() {
        let token = generate_token();
        assert!(!token.is_empty());
        // Should be a valid UUID
        assert!(Uuid::parse_str(&token).is_ok());
    }

    #[test]
    fn test_save_and_load_token() {
        let temp_dir = TempDir::new().unwrap();
        let token_path = temp_dir.path().join("test.token");

        // Generate and save token
        let (token, was_new) = load_or_generate_token(&token_path).unwrap();
        assert!(!token.is_empty());
        assert!(was_new);

        // Load token again
        let (loaded_token, was_new2) = load_or_generate_token(&token_path).unwrap();
        assert_eq!(token, loaded_token);
        assert!(!was_new2);
    }

    #[test]
    fn test_token_file_permissions() {
        let temp_dir = TempDir::new().unwrap();
        let token_path = temp_dir.path().join("test.token");

        let (_token, _was_new) = load_or_generate_token(&token_path).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = fs::metadata(&token_path).unwrap();
            let permissions = metadata.permissions();
            assert_eq!(permissions.mode() & 0o777, 0o600);
        }
    }
}
