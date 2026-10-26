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

# A migration published in a tag never changes: a database that applied it
# refuses another file with the same version. Its down may still be fixed.
tag="$(git tag --list 'v*' --sort=-v:refname | head -n 1)"
if [[ -z "$tag" ]]; then
  echo "no v* tag: fetch the tags to check the published migrations" >&2
  failed=1
fi
while IFS= read -r up; do
  if ! git show "$tag:$up" | cmp -s - "$up"; then
    echo "$up is published in $tag: add a migration instead of changing it" >&2
    failed=1
  fi
done < <([[ -n "$tag" ]] && git ls-tree --name-only "$tag" -- migrations/ | grep '\.up\.sql$')

exit "$failed"
