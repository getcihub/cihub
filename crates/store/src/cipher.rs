use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::hkdf::{HKDF_SHA256, Salt};
use secrecy::{ExposeSecret, SecretString};

use crate::StoreError;
use crate::secret::draw_secret_bytes;

/// The shortest `database.secret`: 32 bytes, the size of the key.
pub const MIN_DATABASE_SECRET_BYTES: usize = 32;

/// Seals the secrets that Postgres keeps, with AES-256-GCM and a key that
/// HKDF-SHA256 derives from `database.secret`: a copy of the database alone
/// opens none of them.
pub struct DatabaseCipher {
    key: LessSafeKey,
}

impl DatabaseCipher {
    /// # Errors
    ///
    /// `secret` is shorter than [`MIN_DATABASE_SECRET_BYTES`].
    pub fn new(secret: &SecretString) -> Result<Self, StoreError> {
        let secret = secret.expose_secret().as_bytes();
        if secret.len() < MIN_DATABASE_SECRET_BYTES {
            return Err(StoreError::ShortDatabaseSecret);
        }

        let pseudo_random = Salt::new(HKDF_SHA256, b"cihub-database-secret").extract(secret);
        let key = pseudo_random
            .expand(&[b"cihub-sealed-column"], &AES_256_GCM)
            .map_err(|_| StoreError::Seal)?;

        Ok(Self {
            key: LessSafeKey::new(UnboundKey::from(key)),
        })
    }

    /// `plain` sealed for `place`, a name of its row and column: a sealed
    /// value copied to another place does not open there.
    ///
    /// # Errors
    ///
    /// The system has no random source for the nonce.
    pub fn seal(&self, plain: &SecretString, place: &str) -> Result<Vec<u8>, StoreError> {
        let nonce = draw_secret_bytes::<NONCE_LEN>()?;
        let mut sealed = plain.expose_secret().as_bytes().to_vec();

        self.key
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(place.as_bytes()),
                &mut sealed,
            )
            .map_err(|_| StoreError::Seal)?;

        Ok([nonce.as_slice(), &sealed].concat())
    }

    /// The value that [`DatabaseCipher::seal`] sealed for `place`.
    ///
    /// # Errors
    ///
    /// Another key or another place sealed it, or it was changed.
    pub fn open(&self, sealed: &[u8], place: &str) -> Result<SecretString, StoreError> {
        let (nonce, rest) = sealed
            .split_at_checked(NONCE_LEN)
            .ok_or(StoreError::Unseal)?;
        let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| StoreError::Unseal)?;
        let mut rest = rest.to_vec();

        let plain = self
            .key
            .open_in_place(nonce, Aad::from(place.as_bytes()), &mut rest)
            .map_err(|_| StoreError::Unseal)?;
        let plain = String::from_utf8(plain.to_vec()).map_err(|_| StoreError::Unseal)?;

        Ok(SecretString::from(plain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cipher(secret: &str) -> DatabaseCipher {
        DatabaseCipher::new(&SecretString::from(secret.to_owned())).unwrap()
    }

    #[test]
    fn a_sealed_value_opens_only_with_its_key_and_its_place() {
        let cipher_a = cipher("a database secret of 32 bytes ab");
        let cipher_b = cipher("a database secret of 32 bytes cd");
        let token = SecretString::from("gho_token".to_owned());

        let sealed = cipher_a.seal(&token, "users.scm_token:user_1").unwrap();
        let again = cipher_a.seal(&token, "users.scm_token:user_1").unwrap();

        assert_ne!(sealed, again, "each seal draws its nonce");
        let opened = cipher_a.open(&sealed, "users.scm_token:user_1").unwrap();
        assert_eq!(opened.expose_secret(), "gho_token");
        assert!(cipher_b.open(&sealed, "users.scm_token:user_1").is_err());
        assert!(cipher_a.open(&sealed, "users.scm_token:user_2").is_err());
        assert!(
            cipher_a
                .open(&sealed[..8], "users.scm_token:user_1")
                .is_err()
        );
    }

    #[test]
    fn a_short_secret_is_refused() {
        let short = SecretString::from("too short".to_owned());

        assert!(matches!(
            DatabaseCipher::new(&short),
            Err(StoreError::ShortDatabaseSecret)
        ));
    }
}
