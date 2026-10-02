//! The HMAC-SHA256 signature of a webhook delivery (`DECISIONS.md` D80).

use ring::hmac;
use secrecy::{ExposeSecret, SecretString};

/// Whether `header`, the value of `X-Hub-Signature-256`, signs `body` with
/// `secret`. The comparison takes the same time whatever the signature.
pub fn verify_signature(secret: &SecretString, body: &[u8], header: Option<&str>) -> bool {
    let Some(tag) = header
        .and_then(|header| header.strip_prefix("sha256="))
        .and_then(decode_hex)
    else {
        return false;
    };
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.expose_secret().as_bytes());
    hmac::verify(&key, body, &tag).is_ok()
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The example of the GitHub documentation on webhook signatures.
    const SIGNATURE: &str =
        "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";

    fn secret() -> SecretString {
        SecretString::from("It's a Secret to Everybody".to_owned())
    }

    #[test]
    fn a_signature_of_the_body_is_accepted() {
        assert!(verify_signature(
            &secret(),
            b"Hello, World!",
            Some(SIGNATURE)
        ));
    }

    #[test]
    fn a_wrong_or_missing_signature_is_refused() {
        assert!(!verify_signature(
            &secret(),
            b"Hello, World?",
            Some(SIGNATURE)
        ));
        let other = SecretString::from("another secret".to_owned());
        assert!(!verify_signature(&other, b"Hello, World!", Some(SIGNATURE)));
        assert!(!verify_signature(&secret(), b"Hello, World!", None));
        assert!(!verify_signature(
            &secret(),
            b"Hello, World!",
            Some("sha1=00")
        ));
        assert!(!verify_signature(
            &secret(),
            b"Hello, World!",
            Some("sha256=zz")
        ));
        assert!(!verify_signature(
            &secret(),
            b"Hello, World!",
            Some("sha256=abc")
        ));
    }
}
