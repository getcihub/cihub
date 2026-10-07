#!/usr/bin/env bash
# Sends the affected targets of the head of a PR to CIHub, for a queue in
# `source: upload` (docs/API.md, DECISIONS.md D234, D236). TARGETS_FILE holds
# a JSON list of strings or the JSON string "ALL".
set -euo pipefail

if [ -z "${UPLOAD_TOKEN:-}" ]; then
  echo "::error::no CIHub token: the route needs a token with the targets:write scope" >&2
  exit 1
fi

url="${UPLOAD_URL%/}/api/v1/repos/${REPOSITORY}/prs/${PR_NUMBER}/affected-targets"
{
  printf '{"head_sha":"%s","targets":' "$HEAD_SHA"
  cat "$TARGETS_FILE"
  printf '}'
} | curl --fail-with-body --silent --show-error --retry 3 --retry-all-errors \
  --header @<(printf 'Authorization: Bearer %s\n' "$UPLOAD_TOKEN") \
  --header 'Content-Type: application/json' \
  --data-binary @- "$url"
