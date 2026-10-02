# `Scm` trait and GitHub adapter

- Every call to the SCM goes through the `Scm` trait, as `Arc<dyn Scm>`, with the connection, the repository and a
  `JobClass` (D76). No HTTP client toward GitHub outside `cihub-github`.
- The core sees no GitHub value: the adapter translates into neutral types (`Mergeability`, `CheckKind`,
  `ScmError`, `ScmEvent`, D23, D78). A GitHub name appears only in `cihub-github`, its `CIHUB_GITHUB_*` variables
  and `docs/GITHUB.md`.
- The simulator (`cihub-scm`, feature `sim`, D73) and the adapter pass the same contract suite. A new trait method
  is added to both and to the suite.
- The recorded GitHub responses (`crates/github/tests/recorded/`) come from the `github` example in `--record`
  mode (D90), which needs the real App and a second account: when a read changes (`PrSnapshot`, a method), say so
  and ask for a new recording rather than write the cassette by hand.
- Webhooks: `POST /webhooks/github` verifies the signature, stores, answers; routing is a separate loop and is
  partial (D83 to D86). Nothing depends on a delivery: the repair loop rereads what matters.
- Budget and modes per connection in `scm_connections` (D77); a 429 pauses the connection (D70).
- Documents: `docs/GITHUB.md`, `docs/RELIABILITY.md`.
