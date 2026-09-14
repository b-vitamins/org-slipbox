#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Usage: tools/github-smoke/live-github.sh OUTPUT_DIR ACCOUNT OWNER/REPOSITORY BRANCH [FOLDER]
#
# Assemble debug and androidTest APKs first. The caller owns the attached device.
# The run needs browser consent for a scratch repository the App already reaches;
# an absent FOLDER means the repository root. ADB, SERIAL, APP_ID, TEST_ID,
# CONSENT_SECONDS and RUN_LIMIT select run inputs. Only the sanitized outcome is
# collected: the device code and the grant stay on the device screen.

set -eu

MODULE=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
ADB=${ADB:-adb}
APP_ID=${APP_ID:-io.github.b_vitamins.slipbox.debug}
TEST_ID=${TEST_ID:-$APP_ID.test}
JOURNEY=io.github.b_vitamins.slipbox.github.manual.LiveProviderJourney
CONSENT_SECONDS=${CONSENT_SECONDS:-300}
RUN_LIMIT=${RUN_LIMIT:-1200}

if [ "$#" -lt 4 ]; then
    echo 'Usage: tools/github-smoke/live-github.sh OUTPUT_DIR ACCOUNT OWNER/REPOSITORY BRANCH [FOLDER]' >&2
    exit 64
fi
OUTPUT=$1
EXPECTED=$2
REPOSITORY=$3
BRANCH=$4
FOLDER=${5:-}

case "$EXPECTED" in
'' | *[!0-9]*)
    echo 'The expected account is GitHub'"'"'s numeric identifier' >&2
    exit 64
    ;;
esac
OWNER=${REPOSITORY%%/*}
NAME=${REPOSITORY#*/}
if [ "$OWNER/$NAME" != "$REPOSITORY" ] || [ -z "$OWNER" ] || [ -z "$NAME" ] ||
    [ "${NAME#*/}" != "$NAME" ]; then
    echo 'The repository is owner/repository' >&2
    exit 64
fi
[ -n "$BRANCH" ] || { echo 'The branch cannot be empty' >&2; exit 64; }

DEBUG_APK=$MODULE/app/build/outputs/apk/debug/app-debug.apk
TEST_APK=$MODULE/app/build/outputs/apk/androidTest/debug/app-debug-androidTest.apk
for apk in "$DEBUG_APK" "$TEST_APK"; do
    [ -f "$apk" ] || { echo "No packaged build at $apk" >&2; exit 64; }
done

RESULTS=/sdcard/Android/media/$APP_ID/live-github
OUTCOME=live-github.json
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

echo "### am instrument -w $TEST_ID/$JOURNEY discover"
if [ -n "$FOLDER" ]; then
    set -- -e folder "$FOLDER"
else
    set --
fi
status=0
bounded "$RUN_LIMIT" device shell am instrument -w \
    -e resultDir "$RESULTS" -e expectedAccount "$EXPECTED" \
    -e repository "$REPOSITORY" -e branch "$BRANCH" \
    -e consentSeconds "$CONSENT_SECONDS" \
    "$@" "$TEST_ID/$JOURNEY" >"$OUTPUT/live-github.log" 2>&1 || status=$?

# Stop the app before returning the device, including timeout paths.
device shell am force-stop "$APP_ID" >/dev/null 2>&1 || true

if device shell test -f "$RESULTS/$OUTCOME"; then
    device pull "$RESULTS/$OUTCOME" "$OUTPUT/$OUTCOME" >/dev/null
    device shell rm -rf "$RESULTS"
    cat "$OUTPUT/$OUTCOME"
else
    echo '### the run left no outcome document on the device'
fi

[ "$status" -ne 124 ] || { echo "FAIL the journey exceeded $RUN_LIMIT seconds"; exit 1; }
if grep -q 'PASS live-github discover' "$OUTPUT/live-github.log"; then
    echo 'PASS live-github discover'
    exit 0
fi
sed -n 's/^INSTRUMENTATION_RESULT: stream=//p' "$OUTPUT/live-github.log"
echo "FAIL live-github discover; the run is recorded in $OUTPUT/live-github.log"
exit 1
