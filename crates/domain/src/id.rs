//! Typed identifiers
//!
//! Text form: `<type>_<32 base32 chars>`, for example
//! `org_0627ank54ajth78mw4353dqpw5dvt5kp`. The 32 characters encode 20 bytes:
//! the 16 identifier bytes, then the big-endian CRC32 (IEEE) of
//! `<type>_<16 identifier bytes>`. The checksum catches typos, truncation and
//! a changed type. The 16 bytes are a ULID, as in the Go package.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ulid::Ulid;

macro_rules! id_type {
    ($($(#[$meta:meta])* $name:ident => $prefix:literal),+ $(,)?) => {$(
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Ulid);

        impl $name {
            const PREFIX: &'static str = $prefix;

            /// Generates a new ULID from the system clock and random bytes.
            #[expect(
                clippy::disallowed_methods,
                reason = "the one place that makes identifiers from the clock and randomness"
            )]
            #[expect(
                clippy::new_without_default,
                reason = "a `Default` would read the clock and randomness past the lint"
            )]
            pub fn new() -> Self {
                Self(Ulid::generate())
            }

            /// The identifier of `ulid`, for a value read back or made by
            /// the caller.
            pub const fn from_ulid(ulid: Ulid) -> Self {
                Self(ulid)
            }

            /// The ULID of the identifier, without its prefix.
            pub const fn as_ulid(&self) -> &Ulid {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&encode(Self::PREFIX, self.0))
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                parse(Self::PREFIX, s).map(Self)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }
    )+};
}

id_type!(
    /// Tenancy boundary: every other record carries an `OrgId`.
    OrgId => "org",
    /// A repository that a connection reaches; a repository that moves to
    /// another connection gets a new one.
    RepoId => "repo",
    /// The queue of one target branch of a repository.
    QueueId => "queue",
    /// One queue item per PR submission; a resubmission creates a new item.
    ItemId => "item",
    /// A test group: one PR, a batch, or a half of a bisection.
    GroupId => "group",
    /// One attempt to test a group; the name of its test branch ends with it.
    RunId => "run",
    /// One event of the log of a queue.
    EventId => "event",
    /// How CIHub reaches the repositories of an SCM account.
    ConnectionId => "connection",
    /// A user account of the SCM, global.
    UserId => "user",
    /// The row of an API token; the secret itself is `cht_…`.
    ApiTokenId => "apitoken",
    /// One entry of the audit log of an organization.
    AuditEntryId => "audit",
    /// A session of a user, global; the secret itself is `chs_…` for the
    /// dashboard, `chc_…` for the command line.
    SessionId => "session",
    /// A sign-in of the command line in progress, global; its secret is
    /// `chd_…`.
    DeviceCodeId => "devicecode",
    /// One API request, for the logs; never stored.
    RequestId => "request",
);

/// Parse errors, in the order the checks run. The first five match the
/// sentinel errors of the Go package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdParseError {
    /// Empty, longer than 83 characters, or a main part that is not 32
    /// characters.
    InvalidSize,
    /// No `_` between the prefix and the main part.
    NoSeparator,
    /// A prefix that is not 1 to 50 ASCII letters.
    InvalidType,
    /// A character of the main part outside the base32 alphabet.
    InvalidId,
    /// The checksum does not match the prefix and the ULID.
    InvalidChecksum,
    /// A valid identifier of another type.
    WrongType {
        /// The prefix that the type expects.
        expected: &'static str,
    },
}

impl fmt::Display for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSize => f.write_str("invalid identifier size"),
            Self::NoSeparator => f.write_str("identifier has no `_` separator"),
            Self::InvalidType => f.write_str("identifier type must be 1 to 50 ASCII letters"),
            Self::InvalidId => {
                f.write_str("identifier has a character outside the base32 alphabet")
            }
            Self::InvalidChecksum => f.write_str("identifier checksum does not match"),
            Self::WrongType { expected } => write!(f, "identifier must start with `{expected}_`"),
        }
    }
}

impl std::error::Error for IdParseError {}

const MAX_TYPE_LEN: usize = 50;
const MAIN_PART_LEN: usize = 32;
const MAX_ID_LEN: usize = MAX_TYPE_LEN + 1 + MAIN_PART_LEN;
pub(crate) const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

fn encode(prefix: &str, ulid: Ulid) -> String {
    let mut data = [0u8; 20];
    data[..16].copy_from_slice(&ulid.to_bytes());
    data[16..].copy_from_slice(&checksum(prefix, ulid).to_be_bytes());

    let mut out = format!("{prefix}_");
    push_base32(&mut out, &data);
    out
}

/// Appends the 32 characters of `data`: standard base32 bit order without
/// padding, 5 bytes give 8 characters.
pub(crate) fn push_base32(out: &mut String, data: &[u8; 20]) {
    for chunk in data.as_chunks::<5>().0 {
        let bits = chunk.iter().fold(0u64, |acc, &b| acc << 8 | u64::from(b));
        for i in (0..8).rev() {
            out.push(char::from(ALPHABET[(bits >> (5 * i) & 0x1f) as usize]));
        }
    }
}

fn parse(prefix: &'static str, s: &str) -> Result<Ulid, IdParseError> {
    if s.is_empty() || s.len() > MAX_ID_LEN {
        return Err(IdParseError::InvalidSize);
    }

    let (typ, main) = s.split_once('_').ok_or(IdParseError::NoSeparator)?;
    if typ.is_empty() || typ.len() > MAX_TYPE_LEN || !typ.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Err(IdParseError::InvalidType);
    }

    if main.len() != MAIN_PART_LEN {
        return Err(IdParseError::InvalidSize);
    }

    let mut data = [0u8; 20];
    for (chunk, out) in main
        .as_bytes()
        .as_chunks::<8>()
        .0
        .iter()
        .zip(data.as_chunks_mut::<5>().0)
    {
        let mut bits = 0u64;
        for &c in chunk {
            let digit = ALPHABET
                .iter()
                .position(|&a| a == c)
                .ok_or(IdParseError::InvalidId)?;
            bits = bits << 5 | digit as u64;
        }
        out.copy_from_slice(&bits.to_be_bytes()[3..]);
    }

    let [bytes @ .., s0, s1, s2, s3] = data;
    let ulid = Ulid::from_bytes(bytes);

    if u32::from_be_bytes([s0, s1, s2, s3]) != checksum(typ, ulid) {
        return Err(IdParseError::InvalidChecksum);
    }

    if typ != prefix {
        return Err(IdParseError::WrongType { expected: prefix });
    }

    Ok(ulid)
}

fn checksum(typ: &str, ulid: Ulid) -> u32 {
    crc32(typ.bytes().chain(*b"_").chain(ulid.to_bytes()))
}

/// CRC32 (IEEE), the same as Go's `crc32.ChecksumIEEE`. Bitwise, because an
/// identifier is about 20 bytes.
fn crc32(bytes: impl Iterator<Item = u8>) -> u32 {
    !bytes.fold(!0u32, |crc, b| {
        (0..8).fold(crc ^ u32::from(b), |c, _| {
            (c >> 1) ^ (0xedb8_8320 & (c & 1).wrapping_neg())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generated with the Go `typedid` package (`NewWithULID` then `String`).
    const GO_VECTORS: [(&str, &str); 12] = [
        (
            "org_00000000000000000000000003d49c8y",
            "00000000000000000000000000",
        ),
        (
            "org_zzzzzzzzzzzzzzzzzzzzzzzzzw4mrf2h",
            "7ZZZZZZZZZZZZZZZZZZZZZZZZZ",
        ),
        (
            "org_0627ank54ajth78mw4353dqpw5dvt5kp",
            "01GHTNCS92MPM9T5710S8VDXQ1",
        ),
        (
            "org_05ywjdspm06ftp1ahh11w4vk086z2kjk",
            "01FQ4KEDN01KYNGAMC88F16WR2",
        ),
        (
            "item_00000000000000000000000003adjj2a",
            "00000000000000000000000000",
        ),
        (
            "item_zzzzzzzzzzzzzzzzzzzzzzzzzw3x3h85",
            "7ZZZZZZZZZZZZZZZZZZZZZZZZZ",
        ),
        (
            "item_0627ank54ajth78mw4353dqpw5aj1vs2",
            "01GHTNCS92MPM9T5710S8VDXQ1",
        ),
        (
            "item_05ywjdspm06ftp1ahh11w4vk081psdr7",
            "01FQ4KEDN01KYNGAMC88F16WR2",
        ),
        (
            "project_000000000000000000000000018kx21w",
            "00000000000000000000000000",
        ),
        (
            "project_zzzzzzzzzzzzzzzzzzzzzzzzzy13c1bk",
            "7ZZZZZZZZZZZZZZZZZZZZZZZZZ",
        ),
        (
            "project_0627ank54ajth78mw4353dqpw78cebtm",
            "01GHTNCS92MPM9T5710S8VDXQ1",
        ),
        (
            "project_05ywjdspm06ftp1ahh11w4vk0a38pxvh",
            "01FQ4KEDN01KYNGAMC88F16WR2",
        ),
    ];

    fn ulid(text: &str) -> Ulid {
        Ulid::from_string(text).unwrap()
    }

    #[test]
    fn matches_go_typedid_vectors() {
        for (text, ulid_text) in GO_VECTORS {
            let (typ, _) = text.split_once('_').unwrap();
            assert_eq!(encode(typ, ulid(ulid_text)), text);
            assert_eq!(parse(typ, text), Ok(ulid(ulid_text)));
        }
        assert_eq!(
            parse("project", "project_0625rt7r8y5fkq9t94zq05msqqmckjv7"),
            Ok(ulid("01GHE6HY27HBWXTEJ97XR1D6DX"))
        );
    }

    #[test]
    fn checksum_is_crc32_ieee() {
        assert_eq!(crc32(b"123456789".iter().copied()), 0xcbf4_3926);
    }

    #[test]
    fn rejects_invalid_text() {
        let long = format!("{}_{}", "a".repeat(51), "0".repeat(32));
        for (text, err) in [
            ("", IdParseError::InvalidSize),
            (
                "org0627ank54ajth78mw4353dqpw5dvt5kp",
                IdParseError::NoSeparator,
            ),
            (
                "0rg_0627ank54ajth78mw4353dqpw5dvt5kp",
                IdParseError::InvalidType,
            ),
            (long.as_str(), IdParseError::InvalidSize),
            (
                "org_0627ank54ajth78mw4353dqpw5dvt5k",
                IdParseError::InvalidSize,
            ),
            (
                "org_0627ank54ajth78mw4353dqpw5dvt5ki",
                IdParseError::InvalidId,
            ),
            (
                "org_0627ANK54AJTH78MW4353DQPW5DVT5KP",
                IdParseError::InvalidId,
            ),
            (
                "org_0627ank54ajth78mw4353dqpw5dvt5kq",
                IdParseError::InvalidChecksum,
            ),
            (
                "item_0627ank54ajth78mw4353dqpw5dvt5kp",
                IdParseError::InvalidChecksum,
            ),
        ] {
            assert_eq!(parse("org", text), Err(err), "{text}");
        }
    }

    #[test]
    fn typed_ids_use_their_prefix() {
        let org = OrgId::from_ulid(ulid("01GHTNCS92MPM9T5710S8VDXQ1"));
        assert_eq!(org.to_string(), "org_0627ank54ajth78mw4353dqpw5dvt5kp");
        assert_eq!("org_0627ank54ajth78mw4353dqpw5dvt5kp".parse(), Ok(org));
        assert_eq!(
            serde_json::to_string(&org).unwrap(),
            r#""org_0627ank54ajth78mw4353dqpw5dvt5kp""#
        );

        let queue = QueueId::from_ulid(ulid("01GHTNCS92MPM9T5710S8VDXQ1")).to_string();
        assert_eq!(
            queue.parse::<ItemId>(),
            Err(IdParseError::WrongType { expected: "item" })
        );
        assert!(serde_json::from_str::<ItemId>(&format!("\"{queue}\"")).is_err());
    }
}
