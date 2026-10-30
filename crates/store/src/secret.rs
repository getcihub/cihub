use rand::TryRng;
use rand::rngs::SysRng;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use crate::StoreError;

/// `N` random bytes for the secret of a token, a session or a code.
///
/// # Errors
///
/// The system has no random source.
pub fn draw_secret_bytes<const N: usize>() -> Result<[u8; N], StoreError> {
    let mut bytes = [0u8; N];
    #[expect(
        clippy::disallowed_methods,
        reason = "the one place that draws the secret of a token"
    )]
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(StoreError::Random)?;
    Ok(bytes)
}

/// The SHA-256 of `secret`, which a table stores in place of the secret.
pub fn secret_hash(secret: &SecretString) -> Vec<u8> {
    Sha256::digest(secret.expose_secret().as_bytes()).to_vec()
}
