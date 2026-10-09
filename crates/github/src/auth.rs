//! The authentication of the GitHub App: its key, and the JWT that asks for
//! the tokens of its installations (`DECISIONS.md` D80).

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use ring::rand::SystemRandom;
use ring::signature::{RSA_PKCS1_SHA256, RsaKeyPair};
use secrecy::{ExposeSecret, SecretString};
use time::{Duration, OffsetDateTime};

/// A JWT is dated this long in the past, against a clock drift with GitHub.
const JWT_DRIFT: Duration = Duration::seconds(60);

/// A JWT lives this long; GitHub accepts 10 minutes at most.
const JWT_LIFETIME: Duration = Duration::minutes(9);

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("the private key is not a PEM block")]
    NotPem,
    #[error("the PEM block `{0}` is not an RSA private key")]
    Label(String),
    #[error("the PEM block is not valid base64")]
    Base64(#[source] base64::DecodeError),
    #[error("the private key is refused: {0}")]
    Rejected(String),
}

#[derive(Debug)]
pub struct AppKey(RsaKeyPair);

impl AppKey {
    /// Reads a PEM key: `RSA PRIVATE KEY` (PKCS#1, as GitHub gives it) or
    /// `PRIVATE KEY` (PKCS#8).
    ///
    /// # Errors
    ///
    /// The text is not such a PEM block, or the key is not valid.
    pub fn from_pem(pem: &SecretString) -> Result<Self, KeyError> {
        let pem = pem.expose_secret().trim();
        let label = pem
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("-----BEGIN "))
            .and_then(|line| line.strip_suffix("-----"))
            .ok_or(KeyError::NotPem)?;
        let footer = format!("-----END {label}-----");
        let body: String = pem
            .lines()
            .skip(1)
            .take_while(|line| line.trim() != footer)
            .map(str::trim)
            .collect();
        let der = STANDARD.decode(body).map_err(KeyError::Base64)?;
        let key = match label {
            "RSA PRIVATE KEY" => RsaKeyPair::from_der(&der),
            "PRIVATE KEY" => RsaKeyPair::from_pkcs8(&der),
            other => return Err(KeyError::Label(other.to_owned())),
        };
        key.map(Self)
            .map_err(|err| KeyError::Rejected(err.to_string()))
    }

    /// A JWT that authenticates the App `app_id` at `now`, in RS256.
    ///
    /// # Errors
    ///
    /// ring cannot sign, which a valid key does not cause.
    pub fn jwt(&self, app_id: &str, now: OffsetDateTime) -> Result<String, KeyError> {
        let input = signing_input(app_id, now);
        let mut signature = vec![0; self.0.public().modulus_len()];
        self.0
            .sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                input.as_bytes(),
                &mut signature,
            )
            .map_err(|err| KeyError::Rejected(err.to_string()))?;
        Ok(format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature)))
    }
}

fn signing_input(app_id: &str, now: OffsetDateTime) -> String {
    let header = serde_json::json!({ "alg": "RS256", "typ": "JWT" });
    let claims = serde_json::json!({
        "iat": (now - JWT_DRIFT).unix_timestamp(),
        "exp": (now + JWT_LIFETIME).unix_timestamp(),
        "iss": app_id,
    });
    format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_jwt_is_dated_in_the_past_and_lives_nine_minutes() {
        let now = OffsetDateTime::from_unix_timestamp(1_789_000_000).unwrap();
        let input = signing_input("12345", now);
        let (header, claims) = input.split_once('.').unwrap();
        let decode = |part: &str| -> serde_json::Value {
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(part).unwrap()).unwrap()
        };

        assert_eq!(
            decode(header),
            serde_json::json!({ "alg": "RS256", "typ": "JWT" })
        );
        assert_eq!(
            decode(claims),
            serde_json::json!({ "iat": 1_788_999_940, "exp": 1_789_000_540, "iss": "12345" })
        );
    }

    #[test]
    fn a_key_that_is_not_an_rsa_pem_block_is_refused() {
        let read = |text: &str| AppKey::from_pem(&SecretString::from(text.to_owned()));

        assert!(matches!(read("not a key"), Err(KeyError::NotPem)));
        let label = read("-----BEGIN EC PRIVATE KEY-----\nAAAA\n-----END EC PRIVATE KEY-----");

        assert!(matches!(label, Err(KeyError::Label(l)) if l == "EC PRIVATE KEY"));
        let base64 = read("-----BEGIN RSA PRIVATE KEY-----\n@@@\n-----END RSA PRIVATE KEY-----");

        assert!(matches!(base64, Err(KeyError::Base64(_))));
        let der = read("-----BEGIN RSA PRIVATE KEY-----\nAAAA\n-----END RSA PRIVATE KEY-----");

        assert!(matches!(der, Err(KeyError::Rejected(_))));
    }
}
