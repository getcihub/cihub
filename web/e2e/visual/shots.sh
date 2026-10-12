#!/usr/bin/env bash
# The views of the dashboard, captured from a build with the API of api.ts (web/e2e/__screenshots__/, which git
# ignores):
#   shots.sh             the working tree, into head/                  (mise run web-shots)
#   shots.sh diff [REF]  REF into base/ (origin/main by default), then  (mise run web-shots-diff [REF])
#                        the working tree into head/, and their differences into diff/
set -euo pipefail

web="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
shots="$web/e2e/__screenshots__"

# Builds the dashboard of the folder $1 into builds/$2, then captures its views into $2/.
build_and_capture() {
  local source="$1" label="$2"

  rm -rf "${shots:?}/$label" "${shots:?}/builds/$label"
  pnpm --dir "$source" exec vite build --outDir "$shots/builds/$label" --emptyOutDir --logLevel warn
  # A view that the build cannot show fails its test; the others are still captured, and compared.
  CIHUB_VISUAL_BUILD="$shots/builds/$label" CIHUB_VISUAL_OUT="$shots/$label" \
    pnpm --dir "$web" exec playwright test --config playwright.visual.config.ts \
    || echo "Some views of $label did not render: the comparison lists them."
}

pnpm --dir "$web" exec playwright install chromium

if [[ "${1:-}" != "diff" ]]; then
  build_and_capture "$web" head
  echo "The views are in $shots/head."
  exit 0
fi

ref="${2:-origin/main}"
worktree="$(mktemp -d)"
cleanup() {
  git -C "$web" worktree remove --force "$worktree" >/dev/null 2>&1 || true
  rm -rf "$worktree"
}
trap cleanup EXIT

git -C "$web" worktree add --detach --quiet "$worktree" "$ref"
if [[ -d "$worktree/web" ]]; then
  pnpm --dir "$worktree/web" install --frozen-lockfile --silent
  build_and_capture "$worktree/web" base
else
  # A ref before the dashboard: every view is new.
  rm -rf "${shots:?}/base" "${shots:?}/builds/base"
  echo "$ref has no dashboard: every view is new."
fi
build_and_capture "$web" head

rm -rf "${shots:?}/diff"
node "$web/e2e/visual/compare.ts" "$shots/base" "$shots/head" "$shots/diff" "\`$ref\` ($(git -C "$web" rev-parse --short "$ref"))"
echo "The images of the views that differ are in $shots/diff."
