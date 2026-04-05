// PKCE (RFC 7636) helpers — mirrors claude-code/services/oauth/crypto.ts

use rand::RngCore;
use sha2::{Digest, Sha256};

/// Encode bytes as base64url without padding (RFC 4648 §5).
fn base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut i = 0;
    while i + 2 < bytes.len() {
        let b = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(TABLE[(b >> 18) as usize] as char);
        out.push(TABLE[((b >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((b >> 6) & 0x3f) as usize] as char);
        out.push(TABLE[(b & 0x3f) as usize] as char);
        i += 3;
    }
    if i + 1 == bytes.len() {
        let b = (bytes[i] as u32) << 16;
        out.push(TABLE[(b >> 18) as usize] as char);
        out.push(TABLE[((b >> 12) & 0x3f) as usize] as char);
    } else if i + 2 == bytes.len() {
        let b = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(TABLE[(b >> 18) as usize] as char);
        out.push(TABLE[((b >> 12) & 0x3f) as usize] as char);
        out.push(TABLE[((b >> 6) & 0x3f) as usize] as char);
    }
    out
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    rand::thread_rng().fill_bytes(&mut buf);
    buf
}

/// Generate a PKCE code verifier: 32 random bytes, base64url-encoded.
pub fn generate_code_verifier() -> String {
    base64url(&random_bytes::<32>())
}

/// Generate a PKCE code challenge: SHA-256 of the verifier, base64url-encoded.
pub fn generate_code_challenge(verifier: &str) -> String {
    let hash = Sha256::digest(verifier.as_bytes());
    base64url(&hash)
}

/// Generate a random state token for CSRF protection.
pub fn generate_state() -> String {
    base64url(&random_bytes::<32>())
}
