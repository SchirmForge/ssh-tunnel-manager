// SSH Tunnel Manager - Known Hosts Module
// Handles SSH host key verification and storage (known_hosts file)

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::Engine;
use russh::keys::{PublicKey, PublicKeyBase64};
use sha2::{Digest, Sha256};
use tracing::{debug, info, warn};

/// Result of host key verification
#[derive(Debug, Clone, PartialEq)]
pub enum VerifyResult {
    /// Host key is in known_hosts and matches
    Trusted,
    /// Host key is not in known_hosts (first connection)
    Unknown,
    /// Host key is in known_hosts but doesn't match (MITM warning!)
    Mismatch {
        expected_fingerprint: String,
        actual_fingerprint: String,
        line_number: usize,
    },
}

/// A single entry in the known_hosts file
#[derive(Debug, Clone)]
struct KnownHostEntry {
    /// Host pattern (e.g., "192.168.1.1" or "[example.com]:2222")
    host_pattern: String,
    /// Key type (e.g., "ssh-ed25519", "ssh-rsa", "ecdsa-sha2-nistp256")
    key_type: String,
    /// Base64-encoded public key
    key_data: String,
    /// Optional comment
    comment: Option<String>,
    /// Line number in file (for error reporting)
    line_number: usize,
}

impl KnownHostEntry {
    /// Parse a single line from known_hosts file
    fn parse(line: &str, line_number: usize) -> Option<Self> {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            return None;
        }

        // Format: host_pattern key_type key_data [comment]
        let parts: Vec<&str> = line.split_whitespace().collect();

        if parts.len() < 3 {
            warn!("Invalid known_hosts entry at line {}: too few fields", line_number);
            return None;
        }

        Some(KnownHostEntry {
            host_pattern: parts[0].to_string(),
            key_type: parts[1].to_string(),
            key_data: parts[2].to_string(),
            comment: parts.get(3).map(|s| s.to_string()),
            line_number,
        })
    }

    /// Format entry for writing to known_hosts file
    fn format(&self) -> String {
        if let Some(comment) = &self.comment {
            format!("{} {} {} {}", self.host_pattern, self.key_type, self.key_data, comment)
        } else {
            format!("{} {} {}", self.host_pattern, self.key_type, self.key_data)
        }
    }

    /// Check if this entry matches the given host and port
    fn matches(&self, host: &str, port: u16) -> bool {
        let pattern = format_host_pattern(host, port);

        // Direct match
        if self.host_pattern == pattern {
            return true;
        }

        // Also check without port for default SSH port (22)
        if port == 22 && self.host_pattern == host {
            return true;
        }

        false
    }

    /// Verify if the provided key matches this entry
    fn verify_key(&self, key: &PublicKey) -> bool {
        // Get the key type string from russh
        let key_type_str = key_type_to_string(key);

        // Check if key types match
        if self.key_type != key_type_str {
            return false;
        }

        // Compare the base64-encoded key data
        let key_base64 = encode_public_key_base64(key);
        self.key_data == key_base64
    }
}

/// Manager for SSH known_hosts file
pub struct KnownHosts {
    path: PathBuf,
    entries: Vec<KnownHostEntry>,
    hash_hostnames: bool,
}

impl KnownHosts {
    /// Load known_hosts from the default location
    pub fn load() -> Result<Self> {
        let path = Self::default_path()?;
        Self::load_from(&path, false)
    }

    /// Load known_hosts from a PathBuf
    pub fn load_from_pathbuf(path: PathBuf, hash_hostnames: bool) -> Result<Self> {
        Self::load_from(&path, hash_hostnames)
    }

    /// Load known_hosts from a specific path
    pub fn load_from(path: &Path, hash_hostnames: bool) -> Result<Self> {
        let mut entries = Vec::new();

        // If file doesn't exist, that's ok - we'll create it on first save
        if path.exists() {
            let file = fs::File::open(path)
                .context(format!("Failed to open known_hosts file: {}", path.display()))?;
            let reader = BufReader::new(file);

            for (line_idx, line_result) in reader.lines().enumerate() {
                let line = line_result.context("Failed to read line from known_hosts")?;
                if let Some(entry) = KnownHostEntry::parse(&line, line_idx + 1) {
                    entries.push(entry);
                }
            }

            debug!("Loaded {} entries from known_hosts: {}", entries.len(), path.display());
        } else {
            info!("Known_hosts file does not exist yet: {}", path.display());
        }

        Ok(KnownHosts {
            path: path.to_path_buf(),
            entries,
            hash_hostnames,
        })
    }

    /// Get the default known_hosts path: ~/.config/ssh-tunnel-manager/known_hosts
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("ssh-tunnel-manager").join("known_hosts"))
    }

    /// Get the system SSH known_hosts path: ~/.ssh/known_hosts
    pub fn ssh_known_hosts_path() -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
        Ok(home_dir.join(".ssh").join("known_hosts"))
    }

    /// Verify a host key against known_hosts
    pub fn verify(&self, host: &str, port: u16, key: &PublicKey) -> VerifyResult {
        // Find matching entries
        let matching_entries: Vec<&KnownHostEntry> = self.entries
            .iter()
            .filter(|e| e.matches(host, port))
            .collect();

        if matching_entries.is_empty() {
            // No entry found - unknown host
            return VerifyResult::Unknown;
        }

        // Check if any matching entry has the correct key
        for entry in &matching_entries {
            if entry.verify_key(key) {
                // Found a matching key - trusted
                return VerifyResult::Trusted;
            }
        }

        // Found host entry but key doesn't match - MITM warning!
        let actual_fingerprint = calculate_fingerprint(key);

        // Get the expected fingerprint from the first matching entry
        let expected_entry = matching_entries[0];
        let expected_fingerprint = format!("(line {} in known_hosts)", expected_entry.line_number);

        VerifyResult::Mismatch {
            expected_fingerprint,
            actual_fingerprint,
            line_number: expected_entry.line_number,
        }
    }

    /// Add a new host key to known_hosts
    pub fn add(&mut self, host: &str, port: u16, key: &PublicKey) -> Result<()> {
        let host_pattern = format_host_pattern(host, port);
        let key_type = key_type_to_string(key);
        let key_data = encode_public_key_base64(key);

        let entry = KnownHostEntry {
            host_pattern,
            key_type,
            key_data,
            comment: None,
            line_number: self.entries.len() + 1,
        };

        self.entries.push(entry);
        info!("Added host key for {}:{} to known_hosts", host, port);

        Ok(())
    }

    /// Save known_hosts to disk
    pub fn save(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create known_hosts directory")?;
        }

        // Write to file
        let mut file = fs::File::create(&self.path)
            .context(format!("Failed to create known_hosts file: {}", self.path.display()))?;

        // Write header comment
        writeln!(file, "# SSH Tunnel Manager - Known Hosts")?;
        writeln!(file, "# Do not edit this file manually unless you know what you're doing")?;
        writeln!(file)?;

        // Write all entries
        for entry in &self.entries {
            writeln!(file, "{}", entry.format())?;
        }

        // Set restrictive permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&self.path, permissions)
                .context("Failed to set known_hosts file permissions")?;
        }

        info!("Saved {} entries to known_hosts: {}", self.entries.len(), self.path.display());

        Ok(())
    }

    /// Get the path to the known_hosts file
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Format host pattern for known_hosts (e.g., "host" or "[host]:port")
fn format_host_pattern(host: &str, port: u16) -> String {
    if port == 22 {
        // Default SSH port - use simple hostname
        host.to_string()
    } else {
        // Non-standard port - use [host]:port format
        format!("[{}]:{}", host, port)
    }
}

/// Convert russh PublicKey to key type string
/// Extract the algorithm name from the public key
fn key_type_to_string(key: &PublicKey) -> String {
    // Parse the key type from the SSH wire format
    // The first 4 bytes are the length, then comes the algorithm name
    let key_bytes = key.public_key_bytes();
    if key_bytes.len() < 4 {
        return "unknown".to_string();
    }

    // Read the length (big-endian u32)
    let len = u32::from_be_bytes([key_bytes[0], key_bytes[1], key_bytes[2], key_bytes[3]]) as usize;
    if key_bytes.len() < 4 + len {
        return "unknown".to_string();
    }

    // Extract the algorithm name
    String::from_utf8_lossy(&key_bytes[4..4 + len]).to_string()
}

/// Encode public key as base64 string (for known_hosts format)
fn encode_public_key_base64(key: &PublicKey) -> String {
    // Use the public_key_base64() method from PublicKeyBase64 trait
    key.public_key_base64()
}

/// Calculate SHA256 fingerprint of a public key
pub fn calculate_fingerprint(key: &PublicKey) -> String {
    let mut hasher = Sha256::new();
    // Use public_key_bytes() from PublicKeyBase64 trait
    hasher.update(key.public_key_bytes());
    let result = hasher.finalize();

    // Format as SHA256:base64
    use base64::engine::general_purpose::STANDARD;
    format!("SHA256:{}", STANDARD.encode(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_format_host_pattern() {
        assert_eq!(format_host_pattern("example.com", 22), "example.com");
        assert_eq!(format_host_pattern("example.com", 2222), "[example.com]:2222");
        assert_eq!(format_host_pattern("192.168.1.1", 22), "192.168.1.1");
        assert_eq!(format_host_pattern("192.168.1.1", 2222), "[192.168.1.1]:2222");
    }

    #[test]
    fn test_known_host_entry_parse() {
        let line = "example.com ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAbc123";
        let entry = KnownHostEntry::parse(line, 1).unwrap();

        assert_eq!(entry.host_pattern, "example.com");
        assert_eq!(entry.key_type, "ssh-ed25519");
        assert_eq!(entry.key_data, "AAAAC3NzaC1lZDI1NTE5AAAAIAbc123");
        assert_eq!(entry.comment, None);
    }

    #[test]
    fn test_known_host_entry_parse_with_port() {
        let line = "[example.com]:2222 ssh-rsa AAAAB3NzaC1yc2EAAAADAQAB";
        let entry = KnownHostEntry::parse(line, 1).unwrap();

        assert_eq!(entry.host_pattern, "[example.com]:2222");
        assert_eq!(entry.key_type, "ssh-rsa");
    }

    #[test]
    fn test_known_host_entry_matches() {
        let entry = KnownHostEntry {
            host_pattern: "example.com".to_string(),
            key_type: "ssh-ed25519".to_string(),
            key_data: "test".to_string(),
            comment: None,
            line_number: 1,
        };

        assert!(entry.matches("example.com", 22));
        assert!(!entry.matches("example.com", 2222));
        assert!(!entry.matches("other.com", 22));
    }

    #[test]
    fn test_known_host_entry_matches_with_port() {
        let entry = KnownHostEntry {
            host_pattern: "[example.com]:2222".to_string(),
            key_type: "ssh-ed25519".to_string(),
            key_data: "test".to_string(),
            comment: None,
            line_number: 1,
        };

        assert!(entry.matches("example.com", 2222));
        assert!(!entry.matches("example.com", 22));
    }

    #[test]
    fn test_known_hosts_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("known_hosts");

        // Create and save
        let mut known_hosts = KnownHosts {
            path: path.clone(),
            entries: vec![],
            hash_hostnames: false,
        };

        // Add a dummy entry
        known_hosts.entries.push(KnownHostEntry {
            host_pattern: "example.com".to_string(),
            key_type: "ssh-ed25519".to_string(),
            key_data: "AAAAC3NzaC1lZDI1NTE5AAAAIAbc123".to_string(),
            comment: Some("test".to_string()),
            line_number: 1,
        });

        known_hosts.save().unwrap();

        // Load and verify
        let loaded = KnownHosts::load_from(&path, false).unwrap();
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].host_pattern, "example.com");
        assert_eq!(loaded.entries[0].key_type, "ssh-ed25519");
    }

    #[test]
    fn test_known_hosts_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("known_hosts_empty");

        // Load from non-existent file
        let known_hosts = KnownHosts::load_from(&path, false).unwrap();
        assert_eq!(known_hosts.entries.len(), 0);
    }
}
