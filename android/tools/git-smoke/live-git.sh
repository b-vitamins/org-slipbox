#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Usage: tools/git-smoke/live-git.sh OUTPUT_DIR [TOKEN_FILE]
#
# Builds and runs an explicitly invoked private-repository clone/fetch journey.
# Secret and repository inputs enter application-private files over stdin; they
# never appear in an instrumentation argument, URL, log, or result document.

set -eu

MODULE=$(unset CDPATH; cd -- "$(dirname -- "$0")/../.." && pwd)
WRAPPER=${SLIPBOX_ANDROID_DEV:-$HOME/.local/bin/slipbox-android-dev}
ADB=${ADB:-adb}
APP_ID=${APP_ID:-io.github.b_vitamins.slipbox.debug}
TEST_ID=${TEST_ID:-$APP_ID.test}
JOURNEY=io.github.b_vitamins.slipbox.git.manual.LiveGitJourney
RUN_LIMIT=${RUN_LIMIT:-300}
PRIVATE_INPUT=/data/user/0/$APP_ID/files/live-git-input

if [ "$#" -lt 1 ] || [ "$#" -gt 2 ]; then
    echo 'Usage: tools/git-smoke/live-git.sh OUTPUT_DIR [TOKEN_FILE]' >&2
    exit 64
fi
OUTPUT=$1
TOKEN_FILE=${2:-$HOME/sync/.env.local.github}
[ -x "$WRAPPER" ] || { echo "No development wrapper at $WRAPPER" >&2; exit 64; }
[ -r "$TOKEN_FILE" ] || { echo "No readable token file at $TOKEN_FILE" >&2; exit 64; }
mkdir -p "$OUTPUT"

device() {
    if [ -n "${SERIAL:-}" ]; then
        "$WRAPPER" -- "$ADB" -s "$SERIAL" "$@"
    else
        "$WRAPPER" -- "$ADB" "$@"
    fi
}

bounded() {
    limit=$1
    shift
    set -m
    "$@" &
    child=$!
    set +m
    waited=0
    while kill -0 "$child" 2>/dev/null; do
        if [ "$waited" -ge "$limit" ]; then
            kill -TERM -- "-$child" 2>/dev/null || kill -TERM "$child" 2>/dev/null || true
            wait "$child" 2>/dev/null || true
            return 124
        fi
        sleep 1
        waited=$((waited + 1))
    done
    wait "$child"
}

# A trap invokes this function even though static analysis sees no call site.
# shellcheck disable=SC2317
cleanup() {
    device shell run-as "$APP_ID" rm -rf "$PRIVATE_INPUT" >/dev/null 2>&1 || true
    device shell am force-stop "$APP_ID" >/dev/null 2>&1 || true
    unset token repository_json remote branch selection
}
trap cleanup EXIT HUP INT TERM

token=$(tr -d '\r\n' <"$TOKEN_FILE")
[ -n "$token" ] || { echo 'The token file is empty' >&2; exit 64; }
repository_json=$(
    printf 'header = "Authorization: Bearer %s"\n' "$token" |
        "$WRAPPER" -- curl --config - --fail --silent --show-error \
            'https://api.github.com/user/repos?visibility=private&sort=updated&per_page=100'
)
selection=$(
    printf '%s' "$repository_json" |
        "$WRAPPER" -- jq -r \
            '[.[] | select(.private == true and .archived == false) | [.clone_url, .default_branch]][0] | @tsv'
)
unset repository_json
remote=${selection%%	*}
branch=${selection#*	}
if [ -z "$remote" ] || [ "$branch" = "$selection" ] || [ -z "$branch" ]; then
    echo 'No usable private repository is visible to the token' >&2
    exit 64
fi

(
    cd "$MODULE"
    "$WRAPPER" -- ./gradlew --no-daemon assembleDebug assembleDebugAndroidTest
)

DEBUG_APK=$MODULE/app/build/outputs/apk/debug/app-debug.apk
TEST_APK=$MODULE/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
device install -r "$DEBUG_APK" >/dev/null
device install -r "$TEST_APK" >/dev/null
device shell run-as "$APP_ID" mkdir -p "$PRIVATE_INPUT"
printf '%s' "$token" | device shell run-as "$APP_ID" tee "$PRIVATE_INPUT/token" >/dev/null
printf '%s' "$remote" | device shell run-as "$APP_ID" tee "$PRIVATE_INPUT/remote" >/dev/null
printf '%s' "$branch" | device shell run-as "$APP_ID" tee "$PRIVATE_INPUT/branch" >/dev/null
device shell run-as "$APP_ID" chmod 600 \
    "$PRIVATE_INPUT/token" "$PRIVATE_INPUT/remote" "$PRIVATE_INPUT/branch"
unset token remote branch selection

RESULTS=/sdcard/Android/media/$APP_ID/live-git
device shell rm -rf "$RESULTS"
run_status=0
bounded "$RUN_LIMIT" device shell am instrument -w \
    -e resultDir "$RESULTS" "$TEST_ID/$JOURNEY" \
    >"$OUTPUT/live-git.log" 2>&1 || run_status=$?

if device shell test -f "$RESULTS/live-git.json"; then
    device exec-out cat "$RESULTS/live-git.json" >"$OUTPUT/live-git.json"
    device shell rm -rf "$RESULTS"
    cat "$OUTPUT/live-git.json"
else
    echo 'The run left no sanitized outcome document on the device' >&2
fi

[ "$run_status" -ne 124 ] || { echo "FAIL the journey exceeded $RUN_LIMIT seconds"; exit 1; }
if grep -q 'PASS live-git private clone/fetch' "$OUTPUT/live-git.log"; then
    echo 'PASS live-git private clone/fetch'
    exit 0
fi
sed -n 's/^INSTRUMENTATION_RESULT: stream=//p' "$OUTPUT/live-git.log"
echo "FAIL live-git; sanitized artifacts are in $OUTPUT"
exit 1
