#!/usr/bin/env bash
# Writes to TARGETS_FILE the names of the Nx projects that the PR affects, as
# a JSON list, or "ALL" when Nx fails (DECISIONS.md D236).
set -uo pipefail

all() {
  echo "::warning::the affected targets are ALL: $1"
  echo '"ALL"' >"$TARGETS_FILE"
  exit 0
}

projects=$(npx --no-install nx show projects --affected --base="$BASE_SHA" --head="$HEAD_SHA" --json) ||
  all "nx show projects failed"
printf '%s' "$projects" | jq --compact-output --exit-status 'if type == "array" then . else error end' \
  >"$TARGETS_FILE" || all "nx did not print a JSON list"
