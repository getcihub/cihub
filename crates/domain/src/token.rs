//! Text forms of the secrets: an API token is `cht_`, a session of the
//! dashboard `chs_`, a session of the command line `chc_` and a device code
//! `chd_`, then 20 random bytes in the base32 alphabet of the identifiers, 36
//! characters. The caller draws the bytes; the store keeps only the SHA-256
//! of the whole text. The prefix lets a secret scanner tell them apart. The
//! user code of a device code is for a person to read and type.

use crate::id::{ALPHABET, push_base32};

/// The prefix of an API token.
pub const API_TOKEN_PREFIX: &str = "cht_";
/// The prefix of a session token of the dashboard.
pub const SESSION_TOKEN_PREFIX: &str = "chs_";
/// The prefix of a session token of the command line.
pub const CLI_SESSION_TOKEN_PREFIX: &str = "chc_";
/// The prefix of a device code, the secret of a sign-in of the command line.
pub const DEVICE_CODE_PREFIX: &str = "chd_";
const TOKEN_LEN: usize = 36;
/// Twenty consonants: no code spells a word, and none is mistaken for a
/// digit (RFC 8628, section 6.1).
const USER_CODE_ALPHABET: &[u8; 20] = b"BCDFGHJKLMNPQRSTVWXZ";
const USER_CODE_HALF: usize = 4;

/// The text of an API token of the random `bytes`.
pub fn format_api_token(bytes: &[u8; 20]) -> String {
    format_token(API_TOKEN_PREFIX, bytes)
}

/// Whether `text` has the form of an API token. It says nothing of its
/// existence: the store looks for its hash.
pub fn is_api_token(text: &str) -> bool {
    has_token_form(API_TOKEN_PREFIX, text)
}

/// The text of a session token of the dashboard of the random `bytes`.
pub fn format_session_token(bytes: &[u8; 20]) -> String {
    format_token(SESSION_TOKEN_PREFIX, bytes)
}

/// Whether `text` has the form of a session token. It says nothing of its
/// existence: the store looks for its hash.
pub fn is_session_token(text: &str) -> bool {
    has_token_form(SESSION_TOKEN_PREFIX, text)
}

/// The text of a session token of the command line of the random `bytes`.
pub fn format_cli_session_token(bytes: &[u8; 20]) -> String {
    format_token(CLI_SESSION_TOKEN_PREFIX, bytes)
}

/// Whether `text` has the form of a session token of the command line. It
/// says nothing of its existence: the store looks for its hash.
pub fn is_cli_session_token(text: &str) -> bool {
    has_token_form(CLI_SESSION_TOKEN_PREFIX, text)
}

/// The text of a device code of the random `bytes`.
pub fn format_device_code(bytes: &[u8; 20]) -> String {
    format_token(DEVICE_CODE_PREFIX, bytes)
}

/// Whether `text` has the form of a device code. It says nothing of its
/// existence: the store looks for its hash.
pub fn is_device_code(text: &str) -> bool {
    has_token_form(DEVICE_CODE_PREFIX, text)
}

/// The user code of 32 random bytes: 8 letters, each from 4 bytes, so that
/// the modulo leaves no visible bias, as `WDJB-MJHT`.
pub fn format_user_code(bytes: &[u8; 32]) -> String {
    let (chunks, _) = bytes.as_chunks::<4>();
    let letters: Vec<char> = chunks
        .iter()
        .map(|chunk| {
            let index = usize::try_from(u32::from_be_bytes(*chunk)).unwrap_or_default();
            char::from(USER_CODE_ALPHABET[index % USER_CODE_ALPHABET.len()])
        })
        .collect();
    let (first, second) = letters.split_at(USER_CODE_HALF);

    format!(
        "{}-{}",
        first.iter().collect::<String>(),
        second.iter().collect::<String>()
    )
}

/// The user code that a person typed, in the form of [`format_user_code`]:
/// the case, the spaces and the dash do not matter. `None` when it cannot
/// be a user code.
pub fn parse_user_code(text: &str) -> Option<String> {
    let letters: Vec<u8> = text
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace() && *byte != b'-')
        .map(|byte| byte.to_ascii_uppercase())
        .collect();

    if letters.len() != 2 * USER_CODE_HALF
        || !letters.iter().all(|byte| USER_CODE_ALPHABET.contains(byte))
    {
        return None;
    }

    let (first, second) = letters.split_at(USER_CODE_HALF);
    Some(format!(
        "{}-{}",
        String::from_utf8_lossy(first),
        String::from_utf8_lossy(second)
    ))
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

    #[test]
    fn the_secrets_of_the_command_line_have_their_own_prefixes() {
        let session = format_cli_session_token(&[0xff; 20]);
        let device = format_device_code(&[0xff; 20]);

        assert_eq!(session, "chc_zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
        assert!(is_cli_session_token(&session));
        assert!(!is_session_token(&session));
        assert!(!is_api_token(&session));
        assert!(is_device_code(&device));
        assert!(!is_cli_session_token(&device));
    }

    #[test]
    fn a_user_code_is_two_groups_of_four_consonants() {
        let mut bytes = [0u8; 32];
        for (index, chunk) in bytes.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            chunk[3] = u8::try_from(index).unwrap();
        }

        assert_eq!(format_user_code(&bytes), "BCDF-GHJK");
        assert_eq!(format_user_code(&[0xff; 32]), "TTTT-TTTT");
    }

    #[test]
    fn a_typed_user_code_reads_whatever_its_case_and_spaces() {
        for typed in ["WDJB-MJHT", "wdjbmjht", " wdjb mjht ", "Wd-Jb-Mj-Ht"] {
            assert_eq!(
                parse_user_code(typed).as_deref(),
                Some("WDJB-MJHT"),
                "{typed}"
            );
        }
        for wrong in ["", "WDJB-MJH", "WDJB-MJHTX", "WAJB-MJHT", "WDJB-MJH1"] {
            assert_eq!(parse_user_code(wrong), None, "{wrong}");
        }
    }
}
