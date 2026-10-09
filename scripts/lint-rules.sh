#!/usr/bin/env bash
# lint-rules.sh: checks the rules of AGENTS.md that clippy and cargo-deny do
# not see. `mise run lint` runs it.
set -euo pipefail
cd "$(dirname "$0")/.."

failed=0

# Every crate starts with a `//!` comment that says its role.
for root in crates/*/src/lib.rs crates/*/src/main.rs; do
  [[ -f "$root" ]] || continue
  if [[ "$(head -c 3 "$root")" != "//!" ]]; then
    echo "$root: the crate does not start with a //! comment" >&2
    failed=1
  fi
done

# Every spec that the repository cites exists.
while IFS= read -r spec; do
  if [[ ! -f "$spec" ]]; then
    echo "$spec is cited but does not exist" >&2
    failed=1
  fi
done < <(git grep --untracked -hoE 'specs/[A-Z_]+\.md' -- ':!CHANGELOG.md' | sort -u)

exit "$failed"
