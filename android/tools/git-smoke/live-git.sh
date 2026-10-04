#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Usage: tools/git-smoke/live-git.sh OUTPUT_DIR [TOKEN_FILE] OWNER/REPOSITORY
#
# Builds and runs an explicitly selected source-to-offline-reading journey.
# Secret and repository inputs enter application-private files over stdin; they
# never appear in an instrumentation argument, URL, log, or result document.

set -eu

MODULE=$(unset CDPATH; cd -- "$(dirname -- "$0")/../.." && pwd)
WRAPPER=${SLIPBOX_ANDROID_DEV:-$HOME/.local/bin/slipbox-android-dev}
ADB=${ADB:-adb}
APP_ID=${APP_ID:-io.github.b_vitamins.slipbox.debug}
TEST_ID=${TEST_ID:-$APP_ID.test}
JOURNEY=io.github.b_vitamins.slipbox.git.manual.LiveGitJourney
OFFLINE_JOURNEY=io.github.b_vitamins.slipbox.git.manual.LiveOfflineGitJourney
RUN_LIMIT=${RUN_LIMIT:-300}
PRIVATE_INPUT=/data/user/0/$APP_ID/files/live-git-input

if [ "$#" -lt 2 ] || [ "$#" -gt 3 ]; then
    echo 'Usage: tools/git-smoke/live-git.sh OUTPUT_DIR [TOKEN_FILE] OWNER/REPOSITORY' >&2
    exit 64
fi
OUTPUT=$1
if [ "$#" -eq 2 ]; then
    TOKEN_FILE=$MODULE/../.env.github.local
    REPOSITORY=$2
else
    TOKEN_FILE=$2
    REPOSITORY=$3
fi
[ -x "$WRAPPER" ] || { echo "No development wrapper at $WRAPPER" >&2; exit 64; }
[ -r "$TOKEN_FILE" ] || { echo "No readable token file at $TOKEN_FILE" >&2; exit 64; }
case "$REPOSITORY" in
    */*) ;;
    *) echo 'The selected repository must be OWNER/REPOSITORY' >&2; exit 64 ;;
esac
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
    device shell pm clear "$APP_ID" >/dev/null 2>&1 || true
    if [ "${airplane_changed:-0}" -eq 1 ]; then
        if [ "${airplane_before:-0}" -eq 1 ]; then
            device shell cmd connectivity airplane-mode enable >/dev/null 2>&1 || true
        else
            device shell cmd connectivity airplane-mode disable >/dev/null 2>&1 || true
        fi
    fi
    unset token repository_json account_json remote branch repository_id account_id selection \
        airplane_before airplane_changed
}
trap cleanup EXIT HUP INT TERM

token=$(tr -d '\r\n' <"$TOKEN_FILE")
[ -n "$token" ] || { echo 'The token file is empty' >&2; exit 64; }
repository_json=$(
    printf 'header = "Authorization: Bearer %s"\n' "$token" |
        "$WRAPPER" -- curl --config - --fail --silent --show-error \
            "https://api.github.com/repos/$REPOSITORY"
)
selection=$(
    printf '%s' "$repository_json" |
        "$WRAPPER" -- jq -r \
            'select(.private == true and .archived == false) | [.clone_url, .default_branch, (.id | tostring)] | .[]'
)
account_json=$(
    printf 'header = "Authorization: Bearer %s"\n' "$token" |
        "$WRAPPER" -- curl --config - --fail --silent --show-error \
            'https://api.github.com/user'
)
account_id=$(printf '%s' "$account_json" | "$WRAPPER" -- jq -r '.id | tostring')
unset repository_json
unset account_json
remote=$(printf '%s\n' "$selection" | sed -n '1p')
branch=$(printf '%s\n' "$selection" | sed -n '2p')
repository_id=$(printf '%s\n' "$selection" | sed -n '3p')
case "$repository_id:$account_id" in
    *[!0-9:]* | :* | *:) echo 'The selected provider identities are invalid' >&2; exit 64 ;;
esac
if [ -z "$remote" ] || [ -z "$branch" ]; then
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
device shell pm clear "$APP_ID" >/dev/null
device shell run-as "$APP_ID" mkdir -p "$PRIVATE_INPUT"
printf '%s' "$token" | device shell run-as "$APP_ID" tee "$PRIVATE_INPUT/token" >/dev/null
printf '%s' "$remote" | device shell run-as "$APP_ID" tee "$PRIVATE_INPUT/remote" >/dev/null
printf '%s' "$branch" | device shell run-as "$APP_ID" tee "$PRIVATE_INPUT/branch" >/dev/null
printf '%s' "$repository_id" |
    device shell run-as "$APP_ID" tee "$PRIVATE_INPUT/repository-id" >/dev/null
printf '%s' "$account_id" |
    device shell run-as "$APP_ID" tee "$PRIVATE_INPUT/account-id" >/dev/null
device shell run-as "$APP_ID" chmod 600 \
    "$PRIVATE_INPUT/token" "$PRIVATE_INPUT/remote" "$PRIVATE_INPUT/branch" \
    "$PRIVATE_INPUT/repository-id" "$PRIVATE_INPUT/account-id"
unset token remote branch repository_id account_id selection

RESULTS=/sdcard/Android/media/$APP_ID/live-git
device shell rm -rf "$RESULTS"
run_status=0
bounded "$RUN_LIMIT" device shell am instrument -w \
    -e resultDir "$RESULTS" "$TEST_ID/$JOURNEY" \
    >"$OUTPUT/live-git.log" 2>&1 || run_status=$?

if device shell test -f "$RESULTS/live-git.json"; then
    device exec-out cat "$RESULTS/live-git.json" >"$OUTPUT/live-git.json"
    cat "$OUTPUT/live-git.json"
else
    echo 'The run left no sanitized outcome document on the device' >&2
fi

[ "$run_status" -ne 124 ] || { echo "FAIL the journey exceeded $RUN_LIMIT seconds"; exit 1; }
if ! grep -q 'PASS live-git source-to-reading' "$OUTPUT/live-git.log"; then
    sed -n 's/^INSTRUMENTATION_RESULT: stream=//p' "$OUTPUT/live-git.log"
    echo "FAIL live-git online phase; sanitized artifacts are in $OUTPUT"
    exit 1
fi

airplane_before=$(device shell settings get global airplane_mode_on | tr -d '\r')
case "$airplane_before" in
    0|1) ;;
    *) echo 'The device did not report a valid airplane-mode state' >&2; exit 1 ;;
esac
device shell cmd connectivity airplane-mode enable >/dev/null
airplane_changed=1
waited=0
while [ "$(device shell settings get global airplane_mode_on | tr -d '\r')" != 1 ]; do
    [ "$waited" -lt 10 ] || { echo 'Airplane mode did not become active' >&2; exit 1; }
    sleep 1
    waited=$((waited + 1))
done
device shell am force-stop "$APP_ID" >/dev/null

offline_status=0
bounded "$RUN_LIMIT" device shell am instrument -w \
    -e resultDir "$RESULTS" "$TEST_ID/$OFFLINE_JOURNEY" \
    >"$OUTPUT/live-git-offline.log" 2>&1 || offline_status=$?
if device shell test -f "$RESULTS/live-git-offline.json"; then
    device exec-out cat "$RESULTS/live-git-offline.json" >"$OUTPUT/live-git-offline.json"
    cat "$OUTPUT/live-git-offline.json"
else
    echo 'The offline run left no sanitized outcome document on the device' >&2
fi
device shell rm -rf "$RESULTS"

[ "$offline_status" -ne 124 ] || { echo "FAIL the offline journey exceeded $RUN_LIMIT seconds"; exit 1; }
if grep -q 'PASS live-git airplane-mode-restart' "$OUTPUT/live-git-offline.log"; then
    echo 'PASS live-git source-to-reading and airplane-mode-restart'
    exit 0
fi
sed -n 's/^INSTRUMENTATION_RESULT: stream=//p' "$OUTPUT/live-git-offline.log"
echo "FAIL live-git offline phase; sanitized artifacts are in $OUTPUT"
exit 1
