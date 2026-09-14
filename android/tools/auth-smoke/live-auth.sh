#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Usage: tools/auth-smoke/live-auth.sh STAGE OUTPUT_DIR [EXPECTED_ACCOUNT]
#
# Assemble debug and androidTest APKs first. The caller owns the attached device.
# Grant requires browser consent and GitHub's numeric account ID; cancel does not.
# ADB, SERIAL, APP_ID, TEST_ID, CONSENT_SECONDS and RUN_LIMIT select run inputs.
# Only the sanitized outcome is collected. Codes remain on the device screen.

set -eu

MODULE=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
ADB=${ADB:-adb}
APP_ID=${APP_ID:-io.github.b_vitamins.slipbox.debug}
TEST_ID=${TEST_ID:-$APP_ID.test}
JOURNEY=io.github.b_vitamins.slipbox.auth.manual.LiveAuthorizationJourney
CONSENT_SECONDS=${CONSENT_SECONDS:-300}
RUN_LIMIT=${RUN_LIMIT:-1200}

if [ "$#" -lt 2 ]; then
    echo 'Usage: tools/auth-smoke/live-auth.sh STAGE OUTPUT_DIR [EXPECTED_ACCOUNT]' >&2
    exit 64
fi
STAGE=$1
OUTPUT=$2
EXPECTED=${3:-}

case "$STAGE" in
grant)
    [ -n "$EXPECTED" ] || { echo 'The grant stage needs an expected account' >&2; exit 64; }
    ;;
cancel) ;;
*)
    echo "Unknown stage: $STAGE" >&2
    exit 64
    ;;
esac
case "$EXPECTED" in
*[!0-9]*)
    echo 'The expected account is GitHub'"'"'s numeric identifier' >&2
    exit 64
    ;;
esac

DEBUG_APK=$MODULE/app/build/outputs/apk/debug/app-debug.apk
TEST_APK=$MODULE/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
for apk in "$DEBUG_APK" "$TEST_APK"; do
    [ -f "$apk" ] || { echo "No packaged build at $apk" >&2; exit 64; }
done

RESULTS=/sdcard/Android/media/$APP_ID/live-auth
OUTCOME=live-auth.json
mkdir -p "$OUTPUT"

device() {
    if [ -n "${SERIAL:-}" ]; then
        "$ADB" -s "$SERIAL" "$@"
    else
        "$ADB" "$@"
    fi
}

# Run the command in its own process group; return 124 on timeout.
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

echo "### install $APP_ID and $TEST_ID"
device install -r "$DEBUG_APK"
device install -r "$TEST_APK"
device shell rm -rf "$RESULTS"

echo "### am instrument -w $TEST_ID/$JOURNEY $STAGE"
if [ "$STAGE" = grant ]; then
    set -- -e expectedAccount "$EXPECTED"
else
    set --
fi
status=0
bounded "$RUN_LIMIT" device shell am instrument -w \
    -e stage "$STAGE" -e resultDir "$RESULTS" -e consentSeconds "$CONSENT_SECONDS" \
    "$@" "$TEST_ID/$JOURNEY" >"$OUTPUT/live-auth.log" 2>&1 || status=$?

# Stop the app before returning the device, including timeout paths.
device shell am force-stop "$APP_ID" >/dev/null 2>&1 || true

if device shell test -f "$RESULTS/$OUTCOME"; then
    device pull "$RESULTS/$OUTCOME" "$OUTPUT/$OUTCOME" >/dev/null
    device shell rm -rf "$RESULTS"
    cat "$OUTPUT/$OUTCOME"
else
    echo "### the run left no outcome document on the device"
fi

[ "$status" -ne 124 ] || { echo "FAIL the journey exceeded $RUN_LIMIT seconds"; exit 1; }
if grep -q "PASS live-auth $STAGE" "$OUTPUT/live-auth.log"; then
    echo "PASS live-auth $STAGE"
    exit 0
fi
sed -n 's/^INSTRUMENTATION_RESULT: stream=//p' "$OUTPUT/live-auth.log"
echo "FAIL live-auth $STAGE; the run is recorded in $OUTPUT/live-auth.log"
exit 1
