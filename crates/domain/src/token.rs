//! Text forms of the secrets: an API token is `cht_`, a session token `chs_`,
//! then 20 random bytes in the base32 alphabet of the identifiers, 36
//! characters. The caller draws the bytes; the store keeps only the SHA-256
//! of the whole text. The prefix lets a secret scanner tell them apart.

use crate::id::{ALPHABET, push_base32};

pub const API_TOKEN_PREFIX: &str = "cht_";
pub const SESSION_TOKEN_PREFIX: &str = "chs_";
const TOKEN_LEN: usize = 36;

pub fn format_api_token(bytes: &[u8; 20]) -> String {
    format_token(API_TOKEN_PREFIX, bytes)
}

/// Whether `text` has the form of an API token. It says nothing of its
/// existence: the store looks for its hash.
pub fn is_api_token(text: &str) -> bool {
    has_token_form(API_TOKEN_PREFIX, text)
}

pub fn format_session_token(bytes: &[u8; 20]) -> String {
    format_token(SESSION_TOKEN_PREFIX, bytes)
}

/// Whether `text` has the form of a session token. It says nothing of its
/// existence: the store looks for its hash.
pub fn is_session_token(text: &str) -> bool {
    has_token_form(SESSION_TOKEN_PREFIX, text)
}

fn format_token(prefix: &str, bytes: &[u8; 20]) -> String {
    let mut token = String::with_capacity(TOKEN_LEN);
    token.push_str(prefix);
    push_base32(&mut token, bytes);
    token
}

fn has_token_form(prefix: &str, text: &str) -> bool {
    text.len() == TOKEN_LEN
        && text
            .strip_prefix(prefix)
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

    #[test]
    fn a_session_token_is_never_an_api_token() {
        let session = format_session_token(&[0xff; 20]);
        let api = format_api_token(&[0xff; 20]);

        assert_eq!(session, "chs_zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
        assert!(is_session_token(&session));
        assert!(!is_api_token(&session));
        assert!(!is_session_token(&api));
    }
}
