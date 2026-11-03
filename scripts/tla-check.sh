#!/usr/bin/env bash
# tla-check.sh: checks the TLA+ models of specs/tla with TLC, an exhaustive
# enumeration of their bounded state space, not a simulation. Each row of
# specs/tla/cases.tsv is one run with its own configuration and log, and its
# expected verdict: `pass`, or the one invariant that the run must violate (a
# mutation that breaks a rule, a bug of the code still open, a limit of the
# design, or a witness that proves a case is reached). An expected violation
# that comes with any other error fails, so that the verdict of one case never
# stands for another. A known bug prints BUG, not PASS: a green run does not
# mean that the code holds every invariant.
#
# Needs java on PATH: `mise run tla:check` runs it with the java of its task. The logs go
# to test-reports/tla/.
set -euo pipefail
cd "$(dirname "$0")/.."

# A published release, not the rolling one, whose asset changes in place. A
# new version is a new checksum, read from the release and checked by hand.
TLA_VERSION=v1.7.4
TLA_SHA256=936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88
jar=target/tla/tla2tools.jar
logs=test-reports/tla

sha256() {
  if command -v sha256sum >/dev/null; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

# A partial or replaced download never becomes the jar.
ensure_jar() {
  [[ -f "$jar" && "$(sha256 "$jar")" == "$TLA_SHA256" ]] && return 0

  mkdir -p "$(dirname "$jar")"
  local tmp
  tmp="$(mktemp "$jar.XXXXXX")"
  curl -fsSL -o "$tmp" "https://github.com/tlaplus/tlaplus/releases/download/$TLA_VERSION/tla2tools.jar"
  if [[ "$(sha256 "$tmp")" != "$TLA_SHA256" ]]; then
    rm -f "$tmp"
    echo "tla2tools.jar $TLA_VERSION does not have its pinned checksum" >&2
    return 1
  fi
  mv "$tmp" "$jar"
}

# The configuration of a case: the base one, with the constants that the case
# sets (`Name=value`, separated by `;`), and with the invariant it must violate
# as the only one. INVARIANTS ends the base file; an override that matches no
# constant fails, so that a renamed constant never runs the base case.
case_config() {
  local base="$1" overrides="$2" expected="$3"
  awk -v overrides="$overrides" -v expected="$expected" '
    BEGIN {
      if (overrides != "-") {
        n = split(overrides, pairs, ";")
        for (i = 1; i <= n; i++) {
          eq = index(pairs[i], "=")
          value[substr(pairs[i], 1, eq - 1)] = substr(pairs[i], eq + 1)
        }
      }
    }
    /^  [A-Za-z]+ = / {
      name = $1
      if (name in value) { print "  " name " = " value[name]; used[name] = 1; next }
    }
    /^INVARIANTS?( |$)/ && expected != "pass" { print "INVARIANT " expected; exit }
    { print }
    END {
      for (name in value) if (!(name in used)) { print "no constant " name > "/dev/stderr"; exit 1 }
    }
  ' "$base"
}

# Both a pass and a counterexample end with the summary of a complete run.
verdict_is() {
  local expected="$1" status="$2" log="$3"
  grep -Eq '^Finished in .+ at \(.+\)$' "$log" || return 1
  grep -Eq '^[0-9,]+ states generated, [0-9,]+ distinct states found, [0-9,]+ states left on queue\.$' "$log" || return 1

  if [[ "$expected" == pass ]]; then
    [[ "$status" == 0 ]] &&
      grep -Fqx 'Model checking completed. No error has been found.' "$log" &&
      grep -Eq ' 0 states left on queue\.$' "$log" &&
      ! grep -q '^Error:' "$log"
    return
  fi

  [[ "$status" == 12 ]] || return 1
  awk -v verdict="Error: Invariant $expected is violated." \
    -v trace='Error: The behavior up to this point is:' '
    /^Error:/ {
      if ($0 == verdict) verdicts++
      else if ($0 == trace) traces++
      else other = 1
    }
    END { exit !(verdicts == 1 && traces == 1 && !other) }
  ' "$log" && grep -q '^State 1: ' "$log"
}

ensure_jar
rm -rf "$logs"
mkdir -p "$logs"

# A malformed row or a name used twice must not skip a case or overwrite the
# log of another; a known bug names its issue and its impact (`safety` or
# `efficiency`), and only a known bug does.
awk -F '\t' '
  /^#/ || NF == 0 { next }
  NF != 7 || seen[$1]++ { print "specs/tla/cases.tsv:" NR ": a malformed or duplicate case" > "/dev/stderr"; exit 1 }
  ($1 ~ /^bug-/) != ($6 != "-") { print "specs/tla/cases.tsv:" NR ": every bug- case names its issue, and only a bug- case does" > "/dev/stderr"; exit 1 }
  ($1 ~ /^bug-/) ? ($7 != "safety" && $7 != "efficiency") : ($7 != "-") { print "specs/tla/cases.tsv:" NR ": a bug- case has the impact safety or efficiency, any other case -" > "/dev/stderr"; exit 1 }
  { count++ }
  END { if (count == 0) exit 1 }
' specs/tla/cases.tsv

# What a verdict that matches its expectation means, by the kind of the case.
outcome_of() {
  local name="$1" expected="$2" issue="$3" impact="$4"
  case "$name" in
    bug-*) echo "BUG   $name ($impact: still violates $expected, $issue)" ;;
    limit-*) echo "LIMIT $name (accepted: violates $expected)" ;;
    witness-*) echo "PASS  $name (case reached: $expected)" ;;
    *) if [[ "$expected" == pass ]]; then
         echo "PASS  $name (no violation)"
       else
         echo "PASS  $name (mutation caught: $expected)"
       fi ;;
  esac
}

failed=0
while IFS=$'\t' read -r name module config overrides expected issue impact; do
  [[ -z "$name" || "$name" == \#* ]] && continue

  cfg="$logs/$name.cfg"
  log="$logs/$name.log"
  states="target/tla/states/$name"
  case_config "specs/tla/$config.cfg" "$overrides" "$expected" > "$cfg"
  status=0
  java -XX:+UseParallelGC -cp "$jar" tlc2.TLC -workers auto \
    -metadir "$states" -config "$cfg" "specs/tla/$module.tla" < /dev/null > "$log" 2>&1 || status=$?
  rm -rf "$states"

  if verdict_is "$expected" "$status" "$log"; then
    outcome_of "$name" "$expected" "$issue" "$impact"
  elif [[ "$name" == bug-* && "$status" == 0 ]]; then
    echo "FAIL  $name: $expected holds now; if $issue is fixed, make this case a fix case" >&2
    failed=1
  else
    echo "FAIL  $name: exit $status, expected $expected; see $log" >&2
    tail -n 40 "$log" >&2
    failed=1
  fi
done < specs/tla/cases.tsv

exit "$failed"
