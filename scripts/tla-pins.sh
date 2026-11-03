#!/usr/bin/env bash
# tla-pins.sh: keeps each TLA+ model of specs/tla next to the code that it
# copies. A model names that code in pin lines:
#
#   \* pin crates/mq-core/src/graph.rs fn insertion_index 0123456789ab
#   \* pin crates/mergequeue/src/scheduler.rs region target-push-waits 0123456789ab
#
# `fn` is a Rust function, up to its closing brace; `region` is the code
# between `// tla: begin <name>` and `// tla: end <name>`. The last field is
# the start of the SHA-256 of that text. `check` fails when the code is gone or
# its text changed: someone reads the model against the new code, changes the
# model when its rule changed, runs `mise run tla:check`, and only then writes
# the new hashes with `update` (`mise run tla:pin`). `check` also fails on a
# model without a pin and on a region that no model pins. It needs no Java:
# `scripts/lint-rules.sh` runs it.
set -euo pipefail
cd "$(dirname "$0")/.."

mode="${1:-check}"
[[ "$mode" == check || "$mode" == update ]] || { echo "usage: $0 [check|update]" >&2; exit 2; }

sha256() {
  if command -v sha256sum >/dev/null; then
    sha256sum | cut -d ' ' -f 1
  else
    shasum -a 256 | cut -d ' ' -f 1
  fi
}

# The text that a pin names, or nothing when the code does not hold it.
code_of() {
  local file="$1" kind="$2" name="$3"
  [[ -f "$file" ]] || return 0
  if [[ "$kind" == fn ]]; then
    awk -v name="$name" '
      !found && $0 ~ "^[[:space:]]*(pub(\\([^)]*\\))? )?(async )?fn " name "[<(]" {
        found = 1
        match($0, /^[[:space:]]*/)
        indent = substr($0, 1, RLENGTH)
      }
      found { print; if ($0 == indent "}") exit }
    ' "$file"
  else
    awk -v name="$name" '
      $0 ~ "// tla: end " name "$" { inside = 0 }
      inside { print }
      $0 ~ "// tla: begin " name "$" { inside = 1 }
    ' "$file"
  fi
}

hash_of() {
  local text
  text="$(code_of "$1" "$2" "$3")"
  [[ -n "$text" ]] || return 0
  printf '%s\n' "$text" | sha256 | cut -c 1-12
}

failed=0
pinned_regions=" "

for model in specs/tla/*.tla; do
  out="$(mktemp)"
  pins=0
  while IFS= read -r line; do
    if [[ "$line" =~ ^\\\*\ pin\ ([^ ]+)\ (fn|region)\ ([^ ]+)\ ([0-9a-f]+)$ ]]; then
      file="${BASH_REMATCH[1]}" kind="${BASH_REMATCH[2]}" name="${BASH_REMATCH[3]}" hash="${BASH_REMATCH[4]}"
      pins=$((pins + 1))
      [[ "$kind" == region ]] && pinned_regions+="$name "
      now="$(hash_of "$file" "$kind" "$name")"
      if [[ -z "$now" ]]; then
        echo "$model: $file has no $kind $name: point the pin at the code that the model copies" >&2
        failed=1
      elif [[ "$now" != "$hash" && "$mode" == check ]]; then
        echo "$model: $kind $name of $file changed: read the model against it, then mise run tla:check and mise run tla:pin" >&2
        failed=1
      fi
      line="\\* pin $file $kind $name ${now:-$hash}"
    fi
    printf '%s\n' "$line" >> "$out"
  done < "$model"

  if (( pins == 0 )); then
    echo "$model: no pin line names the code that the model copies" >&2
    failed=1
  fi
  if [[ "$mode" == update ]]; then
    mv "$out" "$model"
  else
    rm -f "$out"
  fi
done

# A region that no model pins is a marker left behind.
while IFS= read -r region; do
  if [[ "$pinned_regions" != *" $region "* ]]; then
    echo "the region $region of the code has no pin in specs/tla" >&2
    failed=1
  fi
done < <(git grep --untracked -hoE '// tla: begin [a-z0-9-]+' -- 'crates/*.rs' | sed 's|// tla: begin ||' | sort -u)

exit "$failed"
