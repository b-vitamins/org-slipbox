#!/bin/bash
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Usage: tools/ci/verify-device-smoke.sh [--avd NAME] [--image PACKAGE]
#                                      [--apk APK] [--page-size BYTES]
#                                      [--out DIRECTORY]
#                                      [--record-screenshot-baselines]
#
# Requires Bash process groups, an assembled debug APK and ANDROID_HOME
# (or ANDROID_SDK_ROOT).
# DEVICE_SMOKE_* tool, port, result and deadline overrides are described in
# doc/android-ci.org.

set -eu

CI_INPUTS_LIB=1
. "$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)/inputs.sh"

REQUIRED_SUITES="NativeEngineProbeTest EngineAdapterTest SourceCatalogDeviceTest SourceRefreshWorkerDeviceTest"
SECURITY_GATE="$MODULE/app/src/androidTest/security-gate.txt"
VISUAL_GATE="$MODULE/app/src/androidTest/visual-gate.txt"
VISUAL_MIN_API=34

TEST_SOURCE_ROOT="$MODULE/app/src/androidTest/kotlin"

sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
[ -n "$sdk" ] || abort "ANDROID_HOME is not set"

ADB=${DEVICE_SMOKE_ADB:-$sdk/platform-tools/adb}
EMULATOR=${DEVICE_SMOKE_EMULATOR:-$sdk/emulator/emulator}
AVDMANAGER=${DEVICE_SMOKE_AVDMANAGER:-$sdk/cmdline-tools/latest/bin/avdmanager}
AAPT2=${DEVICE_SMOKE_AAPT2:-$sdk/build-tools/$(pin build-tools)/aapt2}
GRADLEW=${DEVICE_SMOKE_GRADLEW:-$MODULE/gradlew}

SERVER_PORT=${DEVICE_SMOKE_SERVER_PORT:-5037}
CONSOLE_PORT=${DEVICE_SMOKE_CONSOLE_PORT:-5554}
DEVICE_PORT=$((CONSOLE_PORT + 1))
SERIAL=emulator-$CONSOLE_PORT

# Gradle's adb client must use this run's isolated server port.
export ANDROID_ADB_SERVER_PORT="$SERVER_PORT"

COMMAND_LIMIT=${DEVICE_SMOKE_COMMAND_LIMIT:-120}
BOOT_LIMIT=${DEVICE_SMOKE_BOOT_LIMIT:-600}
LAUNCH_LIMIT=${DEVICE_SMOKE_LAUNCH_LIMIT:-120}
TEST_LIMIT=${DEVICE_SMOKE_TEST_LIMIT:-2400}
STOP_LIMIT=${DEVICE_SMOKE_STOP_LIMIT:-60}
DATA_PARTITION_MB=${DEVICE_SMOKE_DATA_PARTITION_MB:-4096}
MIN_DATA_FREE_KB=${DEVICE_SMOKE_MIN_DATA_FREE_KB:-524288}
INSTALL_ATTEMPTS=${DEVICE_SMOKE_INSTALL_ATTEMPTS:-3}
LAUNCH_ATTEMPTS=${DEVICE_SMOKE_LAUNCH_ATTEMPTS:-3}

case $DATA_PARTITION_MB in
'' | *[!0-9]*) abort "DEVICE_SMOKE_DATA_PARTITION_MB must be a positive integer" ;;
esac
[ "$DATA_PARTITION_MB" -gt 0 ] ||
    abort "DEVICE_SMOKE_DATA_PARTITION_MB must be a positive integer"
case $MIN_DATA_FREE_KB in
'' | *[!0-9]*) abort "DEVICE_SMOKE_MIN_DATA_FREE_KB must be a positive integer" ;;
esac
[ "$MIN_DATA_FREE_KB" -gt 0 ] ||
    abort "DEVICE_SMOKE_MIN_DATA_FREE_KB must be a positive integer"
case $INSTALL_ATTEMPTS in
'' | *[!0-9]*) abort "DEVICE_SMOKE_INSTALL_ATTEMPTS must be a positive integer" ;;
esac
[ "$INSTALL_ATTEMPTS" -gt 0 ] ||
    abort "DEVICE_SMOKE_INSTALL_ATTEMPTS must be a positive integer"
case $LAUNCH_ATTEMPTS in
'' | *[!0-9]*) abort "DEVICE_SMOKE_LAUNCH_ATTEMPTS must be a positive integer" ;;
esac
[ "$LAUNCH_ATTEMPTS" -gt 0 ] ||
    abort "DEVICE_SMOKE_LAUNCH_ATTEMPTS must be a positive integer"
RESULTS=${DEVICE_SMOKE_RESULTS:-$MODULE/app/build/outputs/androidTest-results/connected}
ADDITIONAL_OUTPUT=${DEVICE_SMOKE_ADDITIONAL_OUTPUT:-$MODULE/app/build/outputs/connected_android_test_additional_output}

avd=slipbox-ci
image=""
apk="$MODULE/app/build/outputs/apk/debug/app-debug.apk"
expected_page_size=""
out="$MODULE/app/build/reports/device-smoke"
record_baselines=0

while [ $# -gt 0 ]; do
    case $1 in
    --avd)
        [ $# -ge 2 ] || abort "--avd needs a name"
        avd=$2
        shift 2
        ;;
    --image)
        [ $# -ge 2 ] || abort "--image needs an SDK package"
        image=$2
        shift 2
        ;;
    --apk)
        [ $# -ge 2 ] || abort "--apk needs a file"
        apk=$2
        shift 2
        ;;
    --page-size)
        [ $# -ge 2 ] || abort "--page-size needs a byte count"
        expected_page_size=$2
        shift 2
        ;;
    --out)
        [ $# -ge 2 ] || abort "--out needs a directory"
        out=$2
        shift 2
        ;;
    --record-screenshot-baselines)
        record_baselines=1
        shift
        ;;
    --)
        shift
        break
        ;;
    -*) abort "unknown option $1" ;;
    *) break ;;
    esac
done
[ $# -eq 0 ] || abort "unexpected argument $1"

qualified_abis=$(declared_abi_targets | cut -d: -f1)
[ -n "$qualified_abis" ] || abort "$MODULE/app/build.gradle.kts declares no qualified ABI"
[ -n "$image" ] || image=$(system_image)

image_abi=${image##*;}
echo "$qualified_abis" | grep -qx "$image_abi" ||
    abort "$image is built for $image_abi, which the APK does not qualify"

[ -d "$TEST_SOURCE_ROOT" ] || abort "$TEST_SOURCE_ROOT does not exist"
test_sources=""
for suite in $REQUIRED_SUITES; do
    found=$(find "$TEST_SOURCE_ROOT" -name "$suite.kt" -type f)
    [ -n "$found" ] || abort "$TEST_SOURCE_ROOT holds no $suite.kt"
    [ "$(echo "$found" | wc -l | tr -d ' ')" -eq 1 ] ||
        abort "$TEST_SOURCE_ROOT holds more than one $suite.kt"
    if [ -z "$test_sources" ]; then
        test_sources=$(dirname "$found")
    elif [ "$test_sources" != "$(dirname "$found")" ]; then
        abort "$REQUIRED_SUITES do not share one instrumentation package"
    fi
done
test_package=$(echo "${test_sources#"$TEST_SOURCE_ROOT/"}" | tr '/' '.')

validate_gate() {
    gate=$1
    [ -f "$gate" ] || abort "$gate does not exist"
    awk '
        NF == 0 { exit 4 }
        !/^[A-Za-z_][A-Za-z0-9_.]*#[A-Za-z_][A-Za-z0-9_]*$/ { exit 2 }
        seen[$0]++ { exit 3 }
        { count++ }
        END { if (count == 0) exit 5 }
    ' "$gate" >/dev/null 2>&1 ||
        abort "$gate contains an invalid or duplicate case"
    while IFS='#' read -r class method; do
        source="$TEST_SOURCE_ROOT/$(echo "$class" | tr . /).kt"
        [ -f "$source" ] || abort "$gate names missing class $class"
        grep -Eq "^[[:space:]]*fun[[:space:]]+$method[[:space:]]*\\(" "$source" ||
            abort "$gate names missing method $class#$method"
    done <"$gate"
}

validate_gate "$SECURITY_GATE"
validate_gate "$VISUAL_GATE"

engine_classes=""
for suite in $REQUIRED_SUITES; do
    case $engine_classes in
    '') engine_classes="$test_package.$suite" ;;
    *) engine_classes="$engine_classes,$test_package.$suite" ;;
    esac
done
security_cases=$(paste -sd, "$SECURITY_GATE")
visual_cases=$(paste -sd, "$VISUAL_GATE")
runner_cases="$engine_classes,$security_cases,$visual_cases"

[ -f "$apk" ] || abort "$apk does not exist; assemble the debug APK before this gate"
for tool in "$ADB" "$EMULATOR" "$AVDMANAGER" "$AAPT2" "$GRADLEW"; do
    [ -x "$tool" ] || abort "$tool is not executable"
done

avd_home=${ANDROID_AVD_HOME:-${ANDROID_USER_HOME:-$HOME/.android}/avd}
export ANDROID_AVD_HOME="$avd_home"

mkdir -p "$out" "$avd_home"
WORK=$(mktemp -d)

owned_server=0
owned_emulator=""
calls=0

fail() {
    echo "FAIL $*"
    exit 1
}

digest() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

OWN_GROUP=$(ps -o pgid= -p $$ 2>/dev/null | tr -d ' ')
[ -n "$OWN_GROUP" ] || fail "this shell reports no process group of its own"

SOCKET_TOOL=""
for candidate in lsof ss netstat; do
    if command -v "$candidate" >/dev/null 2>&1; then
        SOCKET_TOOL=$candidate
        break
    fi
done
[ -n "$SOCKET_TOOL" ] || fail "no lsof, ss or netstat to read the ports this run needs"

listening() {
    case $SOCKET_TOOL in
    lsof) lsof -nP -iTCP:"$1" -sTCP:LISTEN >/dev/null 2>&1 ;;
    ss) ss -ltnH 2>/dev/null | awk -v port="[:.]$1\$" '$4 ~ port { found = 1 } END { exit !found }' ;;
    netstat) netstat -an 2>/dev/null |
        awk -v port="[:.]$1\$" '$NF == "LISTEN" && $4 ~ port { found = 1 } END { exit !found }' ;;
    esac
}

group_of() {
    ps -o pgid= -p "$1" 2>/dev/null | tr -d ' '
}

running() {
    case $(ps -o state= -p "$1" 2>/dev/null | tr -d ' ') in
    "" | Z*) return 1 ;;
    *) return 0 ;;
    esac
}

group_alive() {
    ps -A -o pgid=,pid=,state= 2>/dev/null |
        awk -v group="$1" '$1 == group && $3 !~ /^Z/ { alive = 1 } END { exit !alive }'
}

show_group() {
    ps -A -o pgid=,pid=,state=,comm= 2>/dev/null | awk -v group="$1" '$1 == group' || true
}

# Stop descendants as well as the leader; never signal this script's group.
stop_group() {
    if [ "$1" = "$OWN_GROUP" ]; then
        echo "### refusing to signal this run's own process group $1" >&2
        return 1
    fi
    for signal in TERM KILL; do
        group_alive "$1" || return 0
        kill -"$signal" -- "-$1" 2>/dev/null || true
        waited_stop=0
        while group_alive "$1" && [ "$waited_stop" -lt "$STOP_LIMIT" ]; do
            sleep 1
            waited_stop=$((waited_stop + 1))
        done
    done
    ! group_alive "$1"
}

# Return the command status, 124 on timeout, or 125 without a usable status.
bounded() {
    limit=$1
    shift
    calls=$((calls + 1))
    published=$WORK/status.$calls
    # Monitor mode creates a process group; rename publishes a complete status.
    set -m
    (
        "$@"
        code=$?
        echo "$code" >"$published.partial"
        mv "$published.partial" "$published"
    ) &
    child=$!
    set +m

    waited_bounded=0
    while [ ! -f "$published" ] && running "$child"; do
        if [ "$waited_bounded" -ge "$limit" ]; then
            echo "### a bounded command exceeded $limit seconds" >&2
            child_group=$(group_of "$child")
            if [ "$child_group" != "$child" ]; then
                echo "### the timed-out command reports group [$child_group]" >&2
                echo "$child" >>"$WORK/unstopped"
            elif ! stop_group "$child"; then
                echo "### group $child outlived TERM and KILL" >&2
                show_group "$child" >&2
                echo "$child" >>"$WORK/unstopped"
            fi
            wait "$child" 2>/dev/null || true
            return 124
        fi
        sleep 1
        waited_bounded=$((waited_bounded + 1))
    done
    wait "$child" 2>/dev/null || true

    [ -f "$published" ] || {
        echo "### a bounded command published no status" >&2
        return 125
    }
    code=""
    read -r code <"$published" || code=""
    case $code in
    '' | *[!0-9]*)
        echo "### a bounded command published the unusable status [$code]" >&2
        return 125
        ;;
    esac
    return "$code"
}

adb() {
    bounded "$COMMAND_LIMIT" "$ADB" -P "$SERVER_PORT" "$@"
}

inventory() {
    listed=0
    adb devices >"$WORK/devices" 2>"$WORK/devices.err" || listed=$?
    if [ "$listed" -ne 0 ]; then
        sed 's/^/  /' "$WORK/devices" "$WORK/devices.err"
        fail "adb devices exited $listed, so the attached set is unknown"
    fi
    parsed=0
    awk '
        !header && /^List of devices attached/ { header = 1; next }
        !header { next }
        NF == 0 { next }
        NF == 2 { print $1 " " $2; next }
        { exit 3 }
        END { if (!header) exit 4 }
    ' "$WORK/devices" >"$WORK/attached" || parsed=$?
    [ "$parsed" -eq 0 ] || sed 's/^/  /' "$WORK/devices"
    case $parsed in
    0) ;;
    3) fail "adb devices listed a malformed entry, so the attached set is unknown" ;;
    4) fail "adb devices printed no device list header, so the attached set is unknown" ;;
    *) fail "the inventory parser exited $parsed, so the attached set is unknown" ;;
    esac
}

ask() {
    asked=0
    adb -s "$SERIAL" shell "$@" >"$WORK/answer" || asked=$?
    if [ "$asked" -ne 0 ]; then
        value=""
        return 1
    fi
    value=$(tr -d '\r' <"$WORK/answer")
}

wait_for_package_service() {
    waited=0
    until ask "cmd package path android" && echo "$value" | grep -q '^package:'; do
        running "$owned_emulator" ||
            fail "the emulator exited before its package service became ready"
        [ "$waited" -lt "$BOOT_LIMIT" ] ||
            fail "$SERIAL package service did not become ready within $BOOT_LIMIT seconds"
        sleep 1
        waited=$((waited + 1))
    done
    echo "### $SERIAL package service ready after $waited seconds"
}

cleanup() {
    status=$?
    trap - EXIT INT TERM
    problems=0

    if [ -n "$owned_emulator" ]; then
        if running "$owned_emulator"; then
            [ "$owned_server" -eq 0 ] || adb -s "$SERIAL" emu kill >/dev/null 2>&1 || true
            waited=0
            while running "$owned_emulator" && [ "$waited" -lt "$STOP_LIMIT" ]; do
                sleep 1
                waited=$((waited + 1))
            done
            stop_group "$owned_emulator" || true
        fi
        wait "$owned_emulator" 2>/dev/null || true
        if running "$owned_emulator" || group_alive "$owned_emulator"; then
            echo "### cleanup could not stop the emulator it started, PID $owned_emulator"
            show_group "$owned_emulator"
            problems=$((problems + 1))
        else
            echo "### cleanup stopped the emulator it started, PID $owned_emulator"
        fi
    else
        echo "### cleanup started no emulator"
    fi

    if [ "$owned_server" -eq 1 ]; then
        adb kill-server >/dev/null 2>&1 || true
        waited=0
        while listening "$SERVER_PORT" && [ "$waited" -lt "$STOP_LIMIT" ]; do
            sleep 1
            waited=$((waited + 1))
        done
        if listening "$SERVER_PORT"; then
            echo "### cleanup could not stop the adb server it started on port $SERVER_PORT"
            problems=$((problems + 1))
        else
            echo "### cleanup stopped the adb server it started on port $SERVER_PORT"
        fi
    else
        echo "### cleanup started no adb server"
    fi

    if [ -n "$owned_emulator" ]; then
        for port in "$CONSOLE_PORT" "$DEVICE_PORT"; do
            if listening "$port"; then
                echo "### cleanup left a listener on emulator port $port"
                problems=$((problems + 1))
            fi
        done
    fi

    if [ -f "$WORK/unstopped" ]; then
        while read -r group; do
            if group_alive "$group"; then
                echo "### cleanup left a running command group $group"
                show_group "$group"
                problems=$((problems + 1))
            else
                echo "### the command group $group stopped before this run returned"
            fi
        done <"$WORK/unstopped"
    fi

    rm -rf "$WORK"
    if [ "$problems" -ne 0 ]; then
        echo "### cleanup reported $problems problem(s) after primary status $status"
        [ "$status" -ne 0 ] || status=1
    fi
    exit "$status"
}
trap cleanup EXIT INT TERM

echo "### device smoke: $avd from $image, $test_package on $SERIAL"
echo "### apk $apk"
echo "### apk sha256 $(digest "$apk")"

# Deadlines need each background command in a separate process group.
set -m
(sleep 1) &
grouped=$!
set +m
[ "$(group_of "$grouped")" = "$grouped" ] ||
    fail "this shell gives a background job no process group of its own"
wait "$grouped" 2>/dev/null || true

if [ ! -f "$avd_home/$avd.ini" ]; then
    echo "### avdmanager create avd -n $avd -k $image"
    printf 'no\n' | "$AVDMANAGER" create avd -n "$avd" -p "$avd_home/$avd.avd" -k "$image"
fi

for port in "$SERVER_PORT" "$CONSOLE_PORT" "$DEVICE_PORT"; do
    if listening "$port"; then
        fail "port $port already has a listener this run must not disturb"
    fi
done

echo "### adb -P $SERVER_PORT start-server"
owned_server=1
started=0
adb start-server || started=$?
[ "$started" -eq 0 ] || fail "the adb server on port $SERVER_PORT did not start (status $started)"

inventory
if [ -s "$WORK/attached" ]; then
    sed 's/^/  /' "$WORK/attached"
    fail "the adb server on port $SERVER_PORT already has attached devices"
fi

echo "### emulator -avd $avd -port $CONSOLE_PORT -partition-size $DATA_PARTITION_MB"
set -m
"$EMULATOR" -avd "$avd" -port "$CONSOLE_PORT" -no-window -no-snapshot -no-boot-anim \
    -gpu swiftshader_indirect -memory 2048 -cores 2 -partition-size "$DATA_PARTITION_MB" \
    >"$out/emulator.log" 2>&1 &
owned_emulator=$!
set +m

waited=0
while :; do
    inventory
    state=$(awk -v serial="$SERIAL" '$1 == serial { print $2 }' "$WORK/attached")
    [ "$state" != device ] || break
    running "$owned_emulator" ||
        fail "the emulator exited before attaching; see $out/emulator.log"
    [ "$waited" -lt "$BOOT_LIMIT" ] ||
        fail "$SERIAL did not attach within $BOOT_LIMIT seconds"
    sleep 1
    waited=$((waited + 1))
done
echo "### $SERIAL attached after $waited seconds"

waited=0
until ask getprop sys.boot_completed && [ "$value" = 1 ]; do
    running "$owned_emulator" || fail "the emulator exited during boot; see $out/emulator.log"
    [ "$waited" -lt "$BOOT_LIMIT" ] ||
        fail "$SERIAL did not finish booting within $BOOT_LIMIT seconds"
    sleep 1
    waited=$((waited + 1))
done
echo "### $SERIAL booted after $waited seconds"

ask getconf PAGESIZE || fail "$SERIAL did not answer getconf PAGESIZE"
page_size=$value
ask getprop ro.product.cpu.abi || fail "$SERIAL did not answer its ABI"
abi=$value
ask getprop ro.build.version.sdk || fail "$SERIAL did not answer its API level"
api=$value
echo "### $SERIAL abi $abi page size $page_size api $api"
# The image ABI was qualified before boot; equality also refuses a reused AVD.
[ "$abi" = "$image_abi" ] ||
    fail "$SERIAL runs $abi, not the $image_abi its system image implements"
[ -z "$expected_page_size" ] || [ "$page_size" = "$expected_page_size" ] ||
    fail "$SERIAL reports page size $page_size, not $expected_page_size"
[ "$api" -ge "$(pin min-sdk)" ] ||
    fail "$SERIAL runs API $api, below the declared minimum $(pin min-sdk)"
[ "$api" -ge "$VISUAL_MIN_API" ] ||
    fail "$SERIAL runs API $api, below the visual gate minimum $VISUAL_MIN_API"

wait_for_package_service

adb -s "$SERIAL" logcat -c

echo "### $SERIAL /data capacity before APK installation"
data_status=0
adb -s "$SERIAL" shell df -k /data >"$WORK/data-filesystem.raw" 2>&1 || data_status=$?
tr -d '\r' <"$WORK/data-filesystem.raw" >"$out/data-filesystem.txt"
sed 's/^/  /' "$out/data-filesystem.txt"
[ "$data_status" -eq 0 ] || fail "$SERIAL could not report /data capacity (status $data_status)"
data_free_kb=$(awk 'NR > 1 && NF >= 6 { available = $(NF - 2) } END { print available }' \
    "$out/data-filesystem.txt")
case $data_free_kb in
'' | *[!0-9]*) fail "$SERIAL reported no numeric available-KiB value for /data" ;;
esac
echo "### $SERIAL /data has $data_free_kb KiB available; gate requires $MIN_DATA_FREE_KB KiB"
[ "$data_free_kb" -ge "$MIN_DATA_FREE_KB" ] ||
    fail "$SERIAL has less than $MIN_DATA_FREE_KB KiB available on /data"

"$AAPT2" dump badging "$apk" >"$WORK/badging" 2>"$WORK/badging.err" ||
    fail "aapt2 could not read $apk"
package=$(sed -n "s/^package: name='\([^']*\)'.*/\1/p" "$WORK/badging")
activity=$(sed -n "s/^launchable-activity: name='\([^']*\)'.*/\1/p" "$WORK/badging")
[ -n "$package" ] || fail "$apk declares no package name"
[ -n "$activity" ] || fail "$apk declares no launchable activity"

install_apk() {
    echo "### adb install --no-streaming -r $package"
    install_attempt=1
    while :; do
        installed=0
        adb -s "$SERIAL" install --no-streaming -r "$apk" >"$WORK/install" 2>&1 || installed=$?
        sed 's/^/  /' "$WORK/install"
        [ "$installed" -ne 0 ] || break
        if { grep -Fq "Can't find service: package" "$WORK/install" ||
            grep -Fq "Failure calling service package: Broken pipe" "$WORK/install"; } &&
            [ "$install_attempt" -lt "$INSTALL_ATTEMPTS" ]; then
            echo "### package service became unavailable during install attempt $install_attempt; retrying"
            wait_for_package_service
            install_attempt=$((install_attempt + 1))
            continue
        fi
        fail "installing $apk exited $installed"
    done
    echo "### installed $package on attempt $install_attempt"
}

install_apk

echo "### am start -W -n $package/$activity"
launch_attempt=1
while :; do
    launched=0
    bounded "$LAUNCH_LIMIT" "$ADB" -P "$SERVER_PORT" -s "$SERIAL" shell \
        am start -W -n "$package/$activity" >"$out/launch.txt" 2>&1 || launched=$?
    sed 's/^/  /' "$out/launch.txt"
    if [ "$launched" -eq 0 ] && grep -q '^Status: ok' "$out/launch.txt" &&
        ! grep -q '^Error' "$out/launch.txt"; then
        break
    fi
    if grep -Fq 'Error type 3' "$out/launch.txt" &&
        grep -Fq "Error: Activity class {$package/$activity} does not exist." "$out/launch.txt" &&
        [ "$launch_attempt" -lt "$LAUNCH_ATTEMPTS" ]; then
        echo "### installed activity unavailable during launch attempt $launch_attempt; reinstalling"
        wait_for_package_service
        install_apk
        launch_attempt=$((launch_attempt + 1))
        continue
    fi
    [ "$launched" -eq 0 ] || fail "the launch command exited $launched"
    grep -q '^Status: ok' "$out/launch.txt" || fail "$package did not report a successful launch"
    fail "$package reported a launch error"
done
echo "### launched $package on attempt $launch_attempt"
ask "pidof $package || true" || fail "$SERIAL did not answer which process $package runs as"
[ -n "$value" ] || fail "$package left no process running after its launch"
echo "### $package runs as pid $value"

echo "### gradlew :app:connectedDebugAndroidTest for engine, security and reviewed visual cases"
tests=0
runner_arguments=("-Pandroid.testInstrumentationRunnerArguments.class=$runner_cases")
if [ "$record_baselines" -eq 1 ]; then
    runner_arguments+=(
        "-Pandroid.testInstrumentationRunnerArguments.recordScreenshotBaselines=true"
    )
    echo "### recording screenshot baselines; comparisons are disabled for this local run"
fi
bounded "$TEST_LIMIT" "$GRADLEW" --no-daemon --max-workers=2 --no-build-cache \
    -p "$MODULE" :app:connectedDebugAndroidTest \
    "${runner_arguments[@]}" \
    >"$out/instrumentation.log" 2>&1 || tests=$?
tail -n 20 "$out/instrumentation.log" | sed 's/^/  /'

# Preserve results before the next instrumentation run overwrites them.
if [ -d "$RESULTS" ]; then
    rm -rf "$out/androidTest-results"
    cp -R "$RESULTS" "$out/androidTest-results"
    echo "### archived $(find "$out/androidTest-results" -name '*.xml' | wc -l | tr -d ' ')" \
        "result document(s) under $out/androidTest-results"
else
    echo "### the connected run left no results tree at $RESULTS"
fi
if [ -d "$ADDITIONAL_OUTPUT" ]; then
    rm -rf "$out/visual-evidence"
    cp -R "$ADDITIONAL_OUTPUT" "$out/visual-evidence"
    echo "### archived $(find "$out/visual-evidence" -type f | wc -l | tr -d ' ')" \
        "visual evidence file(s) under $out/visual-evidence"
else
    echo "### the connected run left no visual evidence tree at $ADDITIONAL_OUTPUT"
fi

[ "$tests" -ne 124 ] || fail "the connected tests exceeded $TEST_LIMIT seconds"
[ "$tests" -ne 125 ] || fail "the connected tests published no status"

find "$test_sources" -name '*.kt' -exec awk '
    FNR == 1 { class = FILENAME; sub(/.*\//, "", class); sub(/\.kt$/, "", class) }
    /^[[:space:]]*@Test[[:space:]]*$/ { pending = 1; next }
    pending && match($0, /fun [A-Za-z0-9_]+/) {
        print class "." substr($0, RSTART + 4, RLENGTH - 4)
        pending = 0
    }
' {} + | sort >"$WORK/declared"
[ -s "$WORK/declared" ] || fail "the instrumentation sources declare no test case"
awk -F'#' '{ class = $1; sub(/^.*\./, "", class); print class "." $2 }' \
    "$SECURITY_GATE" | sort >"$WORK/security-declared"
awk -F'#' '{ class = $1; sub(/^.*\./, "", class); print class "." $2 }' \
    "$VISUAL_GATE" | sort >"$WORK/visual-declared"
cat "$WORK/declared" "$WORK/security-declared" "$WORK/visual-declared" |
    sort -u >"$WORK/required"

: >"$WORK/results"
find "$out/androidTest-results" -name '*.xml' -exec cat {} + >"$WORK/results" 2>/dev/null || true
awk '
    match($0, /<testcase name="[^"]*" classname="[^"]*"/) {
        entry = substr($0, RSTART, RLENGTH)
        match(entry, /name="[^"]*"/)
        name = substr(entry, RSTART + 6, RLENGTH - 7)
        match(entry, /classname="[^"]*"/)
        class = substr(entry, RSTART + 11, RLENGTH - 12)
        sub(/.*\./, "", class)
        print class "." name
    }
' "$WORK/results" | sort >"$WORK/executed"
skipped=$(grep -c '<skipped' "$WORK/results" || true)
echo "### required $(wc -l <"$WORK/declared" | tr -d ' ') engine case(s)," \
    "$(wc -l <"$WORK/security-declared" | tr -d ' ') security case(s) and" \
    "$(wc -l <"$WORK/visual-declared" | tr -d ' ') visual case(s);" \
    "executed $(wc -l <"$WORK/executed" | tr -d ' '), skipped $skipped"
absent=$(comm -23 "$WORK/required" "$WORK/executed")
if [ -n "$absent" ]; then
    echo "$absent" | sed 's/^/  /'
    fail "the device executed none of the required case(s) above"
fi
[ "$skipped" -eq 0 ] || fail "the device skipped $skipped required case(s)"

adb -s "$SERIAL" logcat -d -b crash -b main >"$out/logcat.txt"
echo "### logcat lines $(wc -l <"$out/logcat.txt" | tr -d ' ')"
echo "### engine and loader output"
grep -E 'SlipboxNativeEngine|slipbox_android|CANNOT LINK|dlopen|library .* not found' \
    "$out/logcat.txt" || echo "  none"
echo "### crashes and hangs"
if grep -E 'FATAL EXCEPTION|beginning of crash|ANR in|Fatal signal' "$out/logcat.txt"; then
    fail "the device reported a crash or a hang"
fi
echo "  none"

[ "$tests" -eq 0 ] || fail "the connected tests exited $tests"
echo "PASS $package launched and every engine, security and reviewed visual case ran on $abi"
