//! Identifiers of the decisions of `plan`, without randomness: the same
//! snapshot and `now` always give the same identifiers.

use time::OffsetDateTime;
use ulid::Ulid;

use crate::Snapshot;

/// Makes ULIDs whose 48 time bits come from `now` and whose 80 other bits
/// are a hash of the queue, the number of events applied and an index.
pub(crate) struct IdGen {
    timestamp_ms: u64,
    seed: [u8; 24],
    index: u64,
}

impl IdGen {
    pub(crate) fn new(snapshot: &Snapshot, now: OffsetDateTime) -> Self {
        let mut seed = [0; 24];
        seed[..16].copy_from_slice(&snapshot.queue_id.as_ulid().to_bytes());
        seed[16..].copy_from_slice(&snapshot.events_applied.to_be_bytes());
        let ms = now.unix_timestamp_nanos() / 1_000_000;
        Self {
            timestamp_ms: u64::try_from(ms).unwrap_or(0),
            seed,
            index: 0,
        }
    }

    pub(crate) fn next(&mut self) -> Ulid {
        let index = self.index.to_be_bytes();
        self.index += 1;
        let high = fnv1a(0xcbf2_9ce4_8422_2325, &self.seed, &index);
        let low = fnv1a(0x6c62_272e_07bb_0142, &self.seed, &index);
        let random = (u128::from(high) << 64 | u128::from(low)) & ((1 << 80) - 1);
        Ulid::from_parts(self.timestamp_ms, random)
    }
}

/// FNV-1a, 64 bits: stable across Rust versions, unlike `DefaultHasher`.
fn fnv1a(basis: u64, a: &[u8], b: &[u8]) -> u64 {
    a.iter().chain(b).fold(basis, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use cihub_domain::{OrgId, QueueId, QueueMode};

    use super::*;

    fn snapshot(events: u64) -> Snapshot {
        let mut s = Snapshot::new(
            OrgId::from_ulid(Ulid(1)),
            QueueId::from_ulid(Ulid(2)),
            "main".into(),
            QueueMode::Single,
        );
        s.events_applied = events;
        s
    }

    #[test]
    fn same_snapshot_same_ids() {
        let now = OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(10);
        let mut a = IdGen::new(&snapshot(3), now);
        let mut b = IdGen::new(&snapshot(3), now);
        let first = a.next();
        assert_eq!(first, b.next());
        assert_ne!(first, a.next());
        assert_ne!(first, IdGen::new(&snapshot(4), now).next());
        assert_eq!(first.timestamp_ms(), 10_000);
    }
}
