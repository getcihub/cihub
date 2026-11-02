# cihub-mergequeue

- Only the scheduler writes the log of a queue, under its lease. The jobs call the SCM and give their facts back
  through `queue_inbox`; the scheduler never calls the SCM.
- The decisions come from `cihub-mq-core`; this crate loads, writes and shows them. No queue logic here that
  `plan` could hold.
- Specs: `specs/ALGORITHMS.md`, `specs/DOMAIN.md`, `specs/CI.md`, `specs/RELIABILITY.md`.
