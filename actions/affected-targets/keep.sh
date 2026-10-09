#!/usr/bin/env bash
# Keeps the targets of a PR from a fork, which receives no secrets, in
# ARTIFACT_FILE, for the artifact that the `forward` action sends from a
# `workflow_run` workflow (DECISIONS.md D239).
set -euo pipefail

mkdir -p "$(dirname "$ARTIFACT_FILE")"
jq --null-input --compact-output --argjson pr "$PR_NUMBER" --slurpfile targets "$TARGETS_FILE" \
  '{pr_number: $pr, targets: $targets[0]}' >"$ARTIFACT_FILE"
echo "::notice::no CIHub token: the targets are kept in the artifact cihub-affected-targets for the forward action"
