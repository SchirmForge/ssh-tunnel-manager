// SPDX-License-Identifier: Apache-2.0
// Copyright 2025 SSH Tunnel Manager Contributors

//! Support for the tier-2 tests that need a real SSH server.
//!
//! The target host name and every credential live only in
//! `.local/testing/ssh-target.env`, which is gitignored. Nothing here, and no
//! other committed file, may contain them. When that file is absent or an
//! account is not configured, the tests skip instead of failing, so
//! `cargo test` stays safe on a fresh clone.

#![allow(dead_code)] // Each integration test binary uses a different subset.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Values read from `.local/testing/ssh-target.env`.
pub struct LiveTarget {
    values: HashMap<String, String>,
}

static TARGET: OnceLock<Option<LiveTarget>> = OnceLock::new();

/// Repository root, derived from this crate's manifest directory.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

pub fn env_file_path() -> PathBuf {
    repo_root()
        .join(".local")
        .join("testing")
        .join("ssh-target.env")
}

impl LiveTarget {
    /// Load the target config, or `None` if it is not present.
    pub fn get() -> Option<&'static LiveTarget> {
        TARGET
            .get_or_init(|| {
                let path = env_file_path();
                let contents = std::fs::read_to_string(&path).ok()?;

                let mut values = HashMap::new();
                for line in contents.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, value)) = line.split_once('=') {
                        let value = value.trim().trim_matches('"').trim_matches('\'');
                        if !value.is_empty() {
                            values.insert(key.trim().to_string(), value.to_string());
                        }
                    }
                }

                // A target with no host is not usable.
                values
                    .contains_key("SSH_TUNNEL_TEST_HOST")
                    .then_some(LiveTarget { values })
            })
            .as_ref()
    }

    pub fn get_var(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn host(&self) -> &str {
        self.get_var("SSH_TUNNEL_TEST_HOST")
            .expect("host presence is checked at load time")
    }

    pub fn port(&self) -> u16 {
        self.get_var("SSH_TUNNEL_TEST_PORT")
            .and_then(|p| p.parse().ok())
            .unwrap_or(22)
    }

    pub fn forward_host(&self) -> &str {
        self.get_var("SSH_TUNNEL_TEST_FORWARD_HOST")
            .unwrap_or("127.0.0.1")
    }

    pub fn forward_port(&self) -> u16 {
        self.get_var("SSH_TUNNEL_TEST_FORWARD_PORT")
            .and_then(|p| p.parse().ok())
            .unwrap_or(22)
    }

    /// All of `keys` present, or `None`.
    pub fn require(&self, keys: &[&str]) -> Option<Vec<&str>> {
        keys.iter().map(|k| self.get_var(k)).collect()
    }
}

/// How strictly a skipped live test should be treated.
///
/// A skipped test still reports `ok`, so an unconfigured run looks exactly like
/// a passing one. `SSH_TUNNEL_TEST_STRICT` turns that silence into a failure.
/// It has two levels because the two live tiers can support different amounts:
///
/// * unset — skip freely. A fresh clone runs `cargo test` with no setup.
/// * `1` — the target file must exist. An account it does not configure still
///   skips. This is the tier-2 localhost fixture, which can offer key-based
///   authentication but not password or 2FA (those need PAM, hence privilege).
/// * `all` — the target file must exist *and* every account a test asks for
///   must be configured. This is the tier-4 real target, where all three
///   accounts exist and any skip means something is wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strictness {
    Off,
    TargetRequired,
    FullyConfigured,
}

pub fn strictness() -> Strictness {
    match std::env::var("SSH_TUNNEL_TEST_STRICT").as_deref() {
        Ok("all") => Strictness::FullyConfigured,
        Ok("1") | Ok("true") => Strictness::TargetRequired,
        _ => Strictness::Off,
    }
}

/// Resolve the live target and the named variables, or skip the test.
///
/// Prints why it skipped so an unconfigured run is legible rather than silent.
/// Under `SSH_TUNNEL_TEST_STRICT=1` it panics instead: see [`strict`].
#[macro_export]
macro_rules! live_target_or_skip {
    ($keys:expr) => {{
        match $crate::harness::live::LiveTarget::get() {
            None => {
                let reason = format!(
                    "no live SSH target configured ({} absent). \
                     See docs/testing/ssh-target.env.template.",
                    $crate::harness::live::env_file_path().display()
                );
                if $crate::harness::live::strictness() != $crate::harness::live::Strictness::Off {
                    panic!("SSH_TUNNEL_TEST_STRICT is set but {reason}");
                }
                eprintln!("SKIP {}: {reason}", std::any::type_name_of_val(&|| {}));
                return;
            }
            Some(target) => match target.require($keys) {
                None => {
                    let reason = format!(
                        "live target is configured but {:?} is incomplete. \
                         See docs/testing/ssh-target.env.template.",
                        $keys
                    );
                    // Only `all` treats an unconfigured account as a failure:
                    // the tier-2 fixture legitimately cannot provide them.
                    if $crate::harness::live::strictness()
                        == $crate::harness::live::Strictness::FullyConfigured
                    {
                        panic!("SSH_TUNNEL_TEST_STRICT=all but {reason}");
                    }
                    eprintln!("SKIP: {reason}");
                    return;
                }
                Some(values) => (target, values),
            },
        }
    }};
}

/// Generate a TOTP code for the current time step from a base32 secret.
///
/// Mirrors what an authenticator app would produce, so the 2FA retry path fixed
/// in v0.1.10 can be exercised automatically.
pub fn totp_now(base32_secret: &str) -> String {
    totp_at(base32_secret, unix_time())
}

/// Generate a TOTP code for an explicit unix timestamp.
pub fn totp_at(base32_secret: &str, seconds: u64) -> String {
    let key = base32_decode(base32_secret).expect("TOTP secret should be valid base32");
    let counter = seconds / 30;
    let digest = hmac_sha1(&key, &counter.to_be_bytes());

    // Dynamic truncation, RFC 4226 section 5.4.
    let offset = (digest[19] & 0x0f) as usize;
    let binary = ((digest[offset] as u32 & 0x7f) << 24)
        | ((digest[offset + 1] as u32) << 16)
        | ((digest[offset + 2] as u32) << 8)
        | (digest[offset + 3] as u32);

    format!("{:06}", binary % 1_000_000)
}

/// Seconds until the current TOTP step ends.
pub fn totp_seconds_remaining() -> u64 {
    30 - (unix_time() % 30)
}

fn unix_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after the epoch")
        .as_secs()
}

fn base32_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let mut bits = 0u32;
    let mut bit_count = 0u32;
    let mut output = Vec::new();

    for c in input.trim().bytes() {
        if c == b'=' || c == b' ' || c == b'-' {
            continue;
        }
        let upper = c.to_ascii_uppercase();
        let value = ALPHABET.iter().position(|&a| a == upper)? as u32;

        bits = (bits << 5) | value;
        bit_count += 5;

        if bit_count >= 8 {
            bit_count -= 8;
            output.push((bits >> bit_count) as u8);
        }
    }

    Some(output)
}

/// HMAC-SHA1, as required by RFC 4226. Implemented here to avoid pulling a
/// crypto dependency into the daemon's dev-dependencies for test code alone.
fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    const BLOCK: usize = 64;

    let mut padded = [0u8; BLOCK];
    if key.len() > BLOCK {
        padded[..20].copy_from_slice(&sha1(key));
    } else {
        padded[..key.len()].copy_from_slice(key);
    }

    let mut inner_key = [0x36u8; BLOCK];
    let mut outer_key = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        inner_key[i] ^= padded[i];
        outer_key[i] ^= padded[i];
    }

    let mut inner = Vec::with_capacity(BLOCK + message.len());
    inner.extend_from_slice(&inner_key);
    inner.extend_from_slice(message);
    let inner_hash = sha1(&inner);

    let mut outer = Vec::with_capacity(BLOCK + 20);
    outer.extend_from_slice(&outer_key);
    outer.extend_from_slice(&inner_hash);
    sha1(&outer)
}

fn sha1(message: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];

    let mut padded = message.to_vec();
    let bit_len = (message.len() as u64) * 8;
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);

        for (i, &word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_matches_known_vectors() {
        assert_eq!(
            hex(&sha1(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(hex(&sha1(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn hmac_sha1_matches_rfc2202_vectors() {
        assert_eq!(
            hex(&hmac_sha1(&[0x0b; 20], b"Hi There")),
            "b617318655057264e28bc0b6fb378c8ef146be00"
        );
        assert_eq!(
            hex(&hmac_sha1(b"Jefe", b"what do ya want for nothing?")),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        );
    }

    #[test]
    fn base32_decodes_known_value() {
        // "12345678901234567890" is the RFC 6238 test key.
        assert_eq!(
            base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap(),
            b"12345678901234567890".to_vec()
        );
    }

    #[test]
    fn totp_matches_rfc6238_vectors() {
        // RFC 6238 appendix B, SHA-1 column.
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        assert_eq!(totp_at(secret, 59), "287082");
        assert_eq!(totp_at(secret, 1111111109), "081804");
        assert_eq!(totp_at(secret, 1234567890), "005924");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
