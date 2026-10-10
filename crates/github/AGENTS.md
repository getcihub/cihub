# cihub-github

- The only crate that knows GitHub: it translates GitHub values into the neutral ones of `cihub-scm`. A GitHub
  name appears only here, in the `CIHUB_GITHUB_*` variables and in `specs/GITHUB.md`.
- The recorded responses (`tests/recorded/`) come from the `github` example in `record` mode, which needs the real
  App and a second account. When a read changes (`PrSnapshot`, a method), say so and ask for a new recording;
  never write the recording by hand.
- A webhook delivery is only a signal to read the SCM again: nothing depends on it, the repair loop rereads what
  matters.
- Spec: `specs/GITHUB.md`.
