#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

set -eu

MODULE=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BASELINE="$MODULE/app/src/main/baseline-prof.txt"
STARTUP="$MODULE/app/src/main/startup-prof.txt"

failures=0
fail() {
    failures=$((failures + 1))
    echo "FAIL $*"
}

for profile in "$BASELINE" "$STARTUP"; do
    if [ ! -s "$profile" ]; then
        fail "$profile is absent or empty"
        continue
    fi
    lines=$(wc -l <"$profile" | tr -d ' ')
    bytes=$(wc -c <"$profile" | tr -d ' ')
    [ "$lines" -le 5000 ] || fail "$profile has $lines rules, above the 5000-rule review bound"
    [ "$bytes" -le 1048576 ] || fail "$profile has $bytes bytes, above the 1 MiB source bound"
    if grep -Ev '^[HSP]*Lio/github/b_vitamins/slipbox/' "$profile" | grep -q .; then
        fail "$profile contains a rule outside the application package"
    fi
    if grep -q 'Lio/github/b_vitamins/slipbox/benchmark/' "$profile"; then
        fail "$profile contains benchmark-only code"
    fi
    grep -q 'Lio/github/b_vitamins/slipbox/SlipboxAppKt;' "$profile" || \
        fail "$profile does not cover SlipboxApp"
    LC_ALL=C sort -cu "$profile" 2>/dev/null || \
        fail "$profile is not sorted and unique"
done

baseline_lines=$(wc -l <"$BASELINE" | tr -d ' ')
startup_lines=$(wc -l <"$STARTUP" | tr -d ' ')
[ "$startup_lines" -lt "$baseline_lines" ] || \
    fail "startup profile is not a strict subset of the baseline profile"
missing_startup=$(awk '
    NR == FNR { baseline[$0] = 1; next }
    !($0 in baseline) { missing++ }
    END { print missing + 0 }
' "$BASELINE" "$STARTUP")
[ "$missing_startup" -eq 0 ] || \
    fail "startup profile has $missing_startup rule(s) absent from the baseline profile"

for apk in "$@"; do
    [ -f "$apk" ] || fail "$apk does not exist"
    [ -f "$apk" ] || continue
    entries=$(unzip -Z1 "$apk")
    if ! printf '%s\n' "$entries" | grep -qx 'assets/dexopt/baseline.prof'; then
        fail "$apk packages no compiled baseline.prof"
        continue
    fi
    printf '%s\n' "$entries" | grep -qx 'assets/dexopt/baseline.profm' || \
        fail "$apk packages no compiled baseline.profm"
    compiled=$(unzip -p "$apk" assets/dexopt/baseline.prof | wc -c | tr -d ' ')
    [ "$compiled" -gt 0 ] || fail "$apk packages an empty baseline.prof"
    [ "$compiled" -lt 1572864 ] || \
        fail "$apk baseline.prof is $compiled bytes, at or above ART's 1.5 MiB limit"
done

if [ "$failures" -ne 0 ]; then
    echo "FAIL $failures performance profile check(s)"
    exit 1
fi

echo "PASS Android performance profiles are bounded and packaged"
