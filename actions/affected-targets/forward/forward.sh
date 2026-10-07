#!/usr/bin/env bash
# Sends the targets that the run of a PR from a fork kept in ARTIFACT_FILE
# (DECISIONS.md D239). The PR wrote the artifact, so it is read as data only;
# HEAD_SHA comes from the event, so the targets reach only that head.
set -euo pipefail

if ! PR_NUMBER=$(jq --exit-status '.pr_number | select(type == "number" and . > 0 and . == floor)' \
  "$ARTIFACT_FILE"); then
  echo "::error::the artifact has no valid pr_number" >&2
  exit 1
fi
if ! jq --exit-status --compact-output \
  '.targets | select(. == "ALL" or (type == "array" and all(type == "string")))' \
  "$ARTIFACT_FILE" >"$TARGETS_FILE"; then
  echo "::error::the artifact has no valid targets" >&2
  exit 1
fi
export PR_NUMBER
exec "$(dirname "$0")/../upload.sh"
