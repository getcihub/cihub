#!/usr/bin/env bash
# Writes to TARGETS_FILE the Bazel labels that the changed files of the PR
# affect, as a JSON list, or "ALL" when the query fails (DECISIONS.md D236),
# as for a file that no rule holds, or when a file that defines the build
# changes (D238).
set -uo pipefail

all() {
  echo "::warning::the affected targets are ALL: $1"
  echo '"ALL"' >"$TARGETS_FILE"
  exit 0
}

files=$(git diff --name-only --no-renames "$BASE_SHA...$HEAD_SHA") || all "git diff failed"
if [ -z "$files" ]; then
  echo '[]' >"$TARGETS_FILE"
  exit 0
fi
list=()
while IFS= read -r file; do list+=("$file"); done <<<"$files"
for file in "${list[@]}"; do
  # The query sees a changed BUILD file as a file that nothing depends on,
  # so the PR would look disjoint from the targets that it redefines.
  case "${file##*/}" in
    BUILD | BUILD.bazel | *.bzl | MODULE.bazel | MODULE.bazel.lock | WORKSPACE | WORKSPACE.bazel | \
      WORKSPACE.bzlmod | .bazelrc | .bazelversion)
      all "$file defines the build"
      ;;
  esac
done
words=$(printf '"%s" ' "${list[@]}")
labels=$(bazel query --output=label "rdeps(//..., set($words))") || all "bazel query failed"
printf '%s\n' "$labels" | jq --raw-input --slurp --compact-output 'split("\n") | map(select(. != ""))' \
  >"$TARGETS_FILE" || all "jq failed"
