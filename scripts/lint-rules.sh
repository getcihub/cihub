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

# A log message is the name of its event, `<object>.<action>` in snake_case,
# never a sentence nor a format: the data go in the fields
# (`specs/OBSERVABILITY.md`, rule 3). The Perl program prints each call of a
# `tracing` macro whose last argument is not such a name.
# shellcheck disable=SC2016 # a Perl program, which the shell must not expand
log_messages='
  while (/\b(?:trace|debug|info|warn|error)!(\((?:[^()"]++|"(?:[^"\\]|\\.)*+"|(?1))*\))/g) {
    my ($call, $start) = ($1, $-[0]);
    next if $call =~ /"[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)+"\s*,?\s*\)\z/;
    my $line = 1 + (substr($_, 0, $start) =~ tr/\n//);
    print "$ARGV:$line\n";
  }'
while IFS= read -r call; do
  echo "$call: the message of a log is <object>.<action> in snake_case, its last argument" >&2
  failed=1
done < <(git grep --untracked -lzE '(trace|debug|info|warn|error)!' -- 'crates/*.rs' |
  xargs -0 perl -0777 -ne "$log_messages")

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
