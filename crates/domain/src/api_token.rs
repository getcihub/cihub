//! Text form of an API token (`DECISIONS.md` D32): `cht_` then 20 random bytes
//! in the base32 alphabet of the identifiers, 36 characters. The caller draws
//! the bytes; the store keeps only the SHA-256 of the whole text.

use crate::id::{ALPHABET, push_base32};

pub const API_TOKEN_PREFIX: &str = "cht_";
const API_TOKEN_LEN: usize = 36;

pub fn format_api_token(bytes: &[u8; 20]) -> String {
    let mut token = String::with_capacity(API_TOKEN_LEN);
    token.push_str(API_TOKEN_PREFIX);
    push_base32(&mut token, bytes);
    token
}

/// Whether `text` has the form of an API token. It says nothing of its
/// existence: the store looks for its hash.
pub fn is_api_token(text: &str) -> bool {
    text.len() == API_TOKEN_LEN
        && text
            .strip_prefix(API_TOKEN_PREFIX)
            .is_some_and(|rest| rest.bytes().all(|b| ALPHABET.contains(&b)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_the_prefix_and_32_characters() {
        let token = format_api_token(&[0xff; 20]);
        assert_eq!(token, "cht_zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
        assert!(is_api_token(&token));
        assert_eq!(format_api_token(&[0; 20]).len(), 36);
    }

    #[test]
    fn other_texts_are_not_tokens() {
        for text in [
            "",
            "cht_",
            "cht_zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            "tok_zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
            "cht_zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzu",
            "cht_ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ",
        ] {
            assert!(!is_api_token(text), "{text}");
        }
    }
}
