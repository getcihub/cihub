# cihub-scm

- Every call to the SCM goes through the `Scm` trait, as `Arc<dyn Scm>`, with the connection, the repository and
  a `JobClass`.
- The values here are neutral (`Mergeability`, `CheckKind`, `ScmError`, `ScmEvent`): no SCM name.
- The simulator (feature `sim`) and every adapter pass the same contract suite (`src/contract.rs`). A new trait
  method is added to the trait, the simulator, the adapters and the suite.
