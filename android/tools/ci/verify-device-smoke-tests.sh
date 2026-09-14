#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Exercise device-smoke success, failure and cleanup with mock tools.
# No device is touched or port bound; the mock emulator is an owned process.

set -eu

TOOLS=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SMOKE="$TOOLS/verify-device-smoke.sh"
MODULE=$(CDPATH= cd -- "$TOOLS/../.." && pwd)

server_port=${DEVICE_SMOKE_SERVER_PORT:-5037}
console_port=${DEVICE_SMOKE_CONSOLE_PORT:-5554}
serial=emulator-$console_port

avd=slipbox-ci-fixture
image="system-images;android-37.0;google_apis;arm64-v8a"
package=io.github.b_vitamins.slipbox.debug
activity=io.github.b_vitamins.slipbox.SlipboxActivity
test_sources="$MODULE/app/src/androidTest/kotlin/io/github/b_vitamins/slipbox/engine"
test_package=io.github.b_vitamins.slipbox.engine

checked=0
problems=0

check() {
    checked=$((checked + 1))
    if [ "$2" = "$3" ]; then
        echo "ok $1"
    else
        problems=$((problems + 1))
        echo "NOT OK $1: expected [$2], actual [$3]"
    fi
}

reported() {
    grep -Ec "$1" "$work/smoke.log" || true
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

fixture="$work/fixture"
results="$work/results"
out="$work/out"
avd_home="$work/avd"
apk="$work/apk/app-debug.apk"
mkdir -p "$work/bin" "$work/sdk" "$work/apk"
printf 'not a real archive: aapt2 is mocked\n' >"$apk"

cat >"$work/bin/adb" <<'MOCK'
#!/bin/sh
set -eu
while [ $# -gt 0 ]; do
    case $1 in
    -P | -s) shift 2 ;;
    *) break ;;
    esac
done
command=${1:-}
[ $# -eq 0 ] || shift
case $command in
start-server | kill-server | emu) exit 0 ;;
install)
    echo "Success"
    exit 0
    ;;
devices)
    case $(cat "$FIXTURE/devices-mode") in
    error)
        echo "error: protocol fault" >&2
        exit 1
        ;;
    headerless)
        echo "$(cat "$FIXTURE/serial") device"
        ;;
    malformed)
        echo "List of devices attached"
        echo "$(cat "$FIXTURE/serial") device something"
        ;;
    *)
        echo "List of devices attached"
        if [ -f "$FIXTURE/booted" ]; then
            echo "$(cat "$FIXTURE/serial") device"
        fi
        echo ""
        ;;
    esac
    exit 0
    ;;
logcat)
    case ${1:-} in
    -c) exit 0 ;;
    *) cat "$FIXTURE/logcat" ;;
    esac
    exit 0
    ;;
shell)
    case "$*" in
    "getprop sys.boot_completed")
        if [ -f "$FIXTURE/booted" ]; then echo 1; else echo 0; fi
        ;;
    "getconf PAGESIZE") cat "$FIXTURE/pagesize" ;;
    "getprop ro.product.cpu.abi") cat "$FIXTURE/abi" ;;
    "getprop ro.build.version.sdk") cat "$FIXTURE/api" ;;
    "am start"*)
        echo "$*" >"$FIXTURE/launch-record"
        cat "$FIXTURE/am-start"
        ;;
    "pidof "*) cat "$FIXTURE/pid" ;;
    *)
        echo "mock adb was asked an unexpected device command: $*" >&2
        exit 64
        ;;
    esac
    exit 0
    ;;
*)
    echo "mock adb was asked an unexpected command: $command $*" >&2
    exit 64
    ;;
esac
MOCK

cat >"$work/bin/emulator" <<'MOCK'
#!/bin/sh
set -eu
echo "mock emulator: $*"
echo $$ >"$FIXTURE/emulator.pid"
echo "${ANDROID_AVD_HOME:-}" >"$FIXTURE/emulator-avd-home"
if [ ! -f "$FIXTURE/never-boots" ]; then
    (
        sleep 1
        touch "$FIXTURE/booted"
    ) &
fi
exec sleep 900
MOCK

cat >"$work/bin/avdmanager" <<'MOCK'
#!/bin/sh
set -eu
: >"$FIXTURE/avdmanager-record"
name=""
for argument in "$@"; do
    printf '%s\n' "$argument" >>"$FIXTURE/avdmanager-record"
done
while [ $# -gt 0 ]; do
    case $1 in
    -n)
        name=$2
        shift 2
        ;;
    *) shift ;;
    esac
done
cat >/dev/null
fixture_avd_dir=${ANDROID_AVD_HOME:-$ANDROID_USER_HOME/avd}
mkdir -p "$fixture_avd_dir"
[ -z "$name" ] || touch "$fixture_avd_dir/$name.ini"
MOCK

cat >"$work/bin/aapt2" <<'MOCK'
#!/bin/sh
set -eu
cat "$FIXTURE/badging"
MOCK

cat >"$work/bin/gradlew" <<'MOCK'
#!/bin/sh
set -eu
: >"$FIXTURE/gradle-record"
for argument in "$@"; do
    echo "$argument" >>"$FIXTURE/gradle-record"
done
echo "mock gradle: $*"
if [ -f "$FIXTURE/results.xml" ]; then
    mkdir -p "$RESULTS_DIR"
    cp "$FIXTURE/results.xml" "$RESULTS_DIR/TEST-mock.xml"
fi
exit "$(cat "$FIXTURE/gradle-status")"
MOCK
chmod +x "$work/bin/adb" "$work/bin/emulator" "$work/bin/avdmanager" "$work/bin/aapt2" \
    "$work/bin/gradlew"

declared_cases() {
    find "$test_sources" -name '*.kt' -exec awk '
        FNR == 1 { class = FILENAME; sub(/.*\//, "", class); sub(/\.kt$/, "", class) }
        /^[[:space:]]*@Test[[:space:]]*$/ { pending = 1; next }
        pending && match($0, /fun [A-Za-z0-9_]+/) {
            print class "." substr($0, RSTART + 4, RLENGTH - 4)
            pending = 0
        }
    ' {} + | sort
}

results_document() {
    marked=$1
    inner=$2
    {
        echo '<?xml version="1.0" encoding="UTF-8"?>'
        echo "<testsuite name=\"$test_package\" tests=\"$(wc -l <"$work/cases" | tr -d ' ')\">"
        while read -r case_name; do
            class=${case_name%%.*}
            name=${case_name#*.}
            printf '<testcase name="%s" classname="%s.%s" time="0.5"' \
                "$name" "$test_package" "$class"
            if [ "$case_name" = "$marked" ]; then
                printf '>%s</testcase>\n' "$inner"
            else
                printf ' />\n'
            fi
        done <"$work/cases"
        echo '</testsuite>'
    }
}

declared_cases >"$work/cases"
[ -s "$work/cases" ] || {
    echo "FAIL $test_sources declares no instrumentation case to build a fixture from" >&2
    exit 1
}
load_case=$(grep '^NativeEngineProbeTest\.' "$work/cases" | head -n 1)
[ -n "$load_case" ] || {
    echo "FAIL the fixtures need a NativeEngineProbeTest case" >&2
    exit 1
}

reset() {
    rm -rf "$fixture" "$results" "$out"
    mkdir -p "$fixture" "$results" "$out" "$avd_home"
    echo "$serial" >"$fixture/serial"
    echo normal >"$fixture/devices-mode"
    echo 4096 >"$fixture/pagesize"
    echo arm64-v8a >"$fixture/abi"
    echo 37 >"$fixture/api"
    echo 0 >"$fixture/gradle-status"
    echo 4321 >"$fixture/pid"
    : >"$fixture/logcat"
    printf 'Status: ok\nActivity: %s\nTotalTime: 412\n' "$package/$activity" >"$fixture/am-start"
    printf "package: name='%s' versionCode='1' versionName='0.19.0'\n" "$package" \
        >"$fixture/badging"
    printf "launchable-activity: name='%s'  label='Slipbox' icon=''\n" "$activity" \
        >>"$fixture/badging"
    results_document "" "" >"$fixture/results.xml"
    touch "$avd_home/$avd.ini"
}

smoke() {
    status=0
    env ANDROID_HOME="$work/sdk" ANDROID_USER_HOME="$work/android-user" \
        ANDROID_AVD_HOME="${fixture_avd_home-$avd_home}" FIXTURE="$fixture" \
        RESULTS_DIR="$results" \
        DEVICE_SMOKE_ADB="$work/bin/adb" DEVICE_SMOKE_EMULATOR="$work/bin/emulator" \
        DEVICE_SMOKE_AVDMANAGER="$work/bin/avdmanager" DEVICE_SMOKE_AAPT2="$work/bin/aapt2" \
        DEVICE_SMOKE_GRADLEW="$work/bin/gradlew" DEVICE_SMOKE_RESULTS="$results" \
        DEVICE_SMOKE_SERVER_PORT="$server_port" DEVICE_SMOKE_CONSOLE_PORT="$console_port" \
        DEVICE_SMOKE_COMMAND_LIMIT=15 DEVICE_SMOKE_BOOT_LIMIT=8 \
        DEVICE_SMOKE_LAUNCH_LIMIT=15 DEVICE_SMOKE_TEST_LIMIT=30 DEVICE_SMOKE_STOP_LIMIT=2 \
        "$SMOKE" --avd "$avd" --image "$image" --apk "$apk" --out "$out" "$@" \
        >"$work/smoke.log" 2>&1 || status=$?
}

emulator_stopped() {
    if [ ! -f "$fixture/emulator.pid" ]; then
        echo "started none"
    elif kill -0 "$(cat "$fixture/emulator.pid")" 2>/dev/null; then
        echo "still running"
    else
        echo stopped
    fi
}

reset
smoke --page-size 4096
check "a booted device that runs every declared case passes" 0 "$status"
check "the passing run reports the device it qualified" 1 \
    "$(reported "^### $serial abi arm64-v8a page size 4096 api 37\$")"
check "the passing run reports the digest of the APK it installed" 1 \
    "$(reported "^### apk sha256 $(shasum -a 256 "$apk" | cut -d' ' -f1)\$")"
check "the passing run launches the activity the APK declares" \
    "am start -W -n $package/$activity" "$(cat "$fixture/launch-record")"
check "the passing run scopes the instrumentation to the reused suites" 1 \
    "$(grep -c "^-Pandroid.testInstrumentationRunnerArguments.package=$test_package\$" \
        "$fixture/gradle-record" || true)"
check "the build runs without a daemon, cache or extra workers" "3" \
    "$(grep -Ec '^(--no-daemon|--max-workers=2|--no-build-cache)$' "$fixture/gradle-record" || true)"
check "the passing run archives the machine-readable results" 1 \
    "$(find "$out/androidTest-results" -name '*.xml' | wc -l | tr -d ' ')"
check "the passing run accounts for every declared case" 1 \
    "$(reported "^### declared $(wc -l <"$work/cases" | tr -d ' ') case\(s\), executed $(wc -l <"$work/cases" | tr -d ' '), skipped 0\$")"
check "the passing run stops the emulator it started" stopped "$(emulator_stopped)"
check "the passing run stops the adb server it started" 1 \
    "$(reported '^### cleanup stopped the adb server it started')"
check "the passing run ends in a pass" 1 "$(reported '^PASS ')"

# Simulate a packaged-library load failure in both instrumentation and logcat.
reset
results_document "$load_case" \
    '<failure message="java.lang.UnsatisfiedLinkError">dlopen failed</failure>' \
    >"$fixture/results.xml"
echo 1 >"$fixture/gradle-status"
{
    echo "01-01 00:00:00.000  1000  1000 E linker  : CANNOT LINK EXECUTABLE: library \"libslipbox_android.so\" not found"
    echo "01-01 00:00:00.001  1000  1000 E AndroidRuntime: FATAL EXCEPTION: main"
} >"$fixture/logcat"
smoke --page-size 4096
check "a packaged library that does not load fails the gate" 1 "$status"
check "the failing load is reported as loader output" 1 "$(reported 'CANNOT LINK')"
check "the failing load blocks the gate on the crash it caused" 1 \
    "$(reported '^FAIL the device reported a crash or a hang$')"
check "the failing load reaches no pass" 0 "$(reported '^PASS ')"
check "the failing load still stops the emulator" stopped "$(emulator_stopped)"

reset
echo 1 >"$fixture/gradle-status"
smoke
check "a nonzero instrumentation status fails the gate" 1 "$status"
check "the nonzero instrumentation status is reported" 1 \
    "$(reported '^FAIL the connected tests exited 1$')"

reset
rm "$fixture/results.xml"
smoke
check "an instrumentation run that publishes no results fails the gate" 1 "$status"
check "the empty run is reported as executing none of the declared cases" 1 \
    "$(reported '^FAIL the device executed none of the declared case\(s\) above$')"

reset
results_document "$load_case" '<skipped />' >"$fixture/results.xml"
smoke
check "a skipped case fails the gate" 1 "$status"
check "the skipped case is counted" 1 "$(reported '^FAIL the device skipped 1 declared case\(s\)$')"

reset
grep -v "^$load_case\$" "$work/cases" >"$work/cases.kept"
mv "$work/cases" "$work/cases.all"
mv "$work/cases.kept" "$work/cases"
results_document "" "" >"$fixture/results.xml"
mv "$work/cases.all" "$work/cases"
smoke
check "a declared case the device never ran fails the gate" 1 "$status"
check "the absent case is named" 1 "$(reported "^  $load_case\$")"

reset
smoke --page-size 16384
check "a device with the wrong page size fails the gate" 1 "$status"
check "the page size mismatch is reported" 1 \
    "$(reported '^FAIL .* reports page size 4096, not 16384$')"

reset
echo x86_64 >"$fixture/abi"
smoke
check "a device that is not its image's ABI fails the gate" 1 "$status"
check "the mismatched device ABI is reported" 1 \
    "$(reported '^FAIL .* runs x86_64, not the arm64-v8a its system image implements$')"

reset
smoke_image=$image
image="system-images;android-37.0;google_apis;riscv64"
smoke
image=$smoke_image
check "an unqualified system image is refused" 1 "$status"
check "the unqualified image is refused before any emulator starts" "started none" \
    "$(emulator_stopped)"
check "the unqualified image is named" 1 \
    "$(reported '^FAIL .*riscv64, which the APK does not qualify$')"

# The hosted runner selects this pair; the local ARM64 device run selects the other.
reset
echo x86_64 >"$fixture/abi"
smoke_image=$image
image="system-images;android-37.0;google_apis;x86_64"
smoke --page-size 4096
image=$smoke_image
check "the qualified x86_64 image and device pass the same gate" 0 "$status"
check "the x86_64 run reports the device it qualified" 1 \
    "$(reported "^### $serial abi x86_64 page size 4096 api 37\$")"

reset
printf 'Status: 0\nError: Activity not started, unable to resolve Intent\n' >"$fixture/am-start"
smoke
check "an application that does not launch fails the gate" 1 "$status"
check "the failed launch is reported" 1 \
    "$(reported '^FAIL .* did not report a successful launch$')"

reset
: >"$fixture/pid"
smoke
check "an application that leaves no process fails the gate" 1 "$status"
check "the absent process is reported" 1 \
    "$(reported '^FAIL .* left no process running after its launch$')"

reset
touch "$fixture/never-boots"
smoke
check "an emulator that never attaches fails the gate" 1 "$status"
check "the boot deadline is reported" 1 "$(reported '^FAIL .* did not attach within 8 seconds$')"
check "the deadline stops the emulator it started" stopped "$(emulator_stopped)"

reset
echo error >"$fixture/devices-mode"
smoke
check "an adb server that cannot list devices fails the gate" 1 "$status"
check "the unknown device set is reported" 1 "$(reported 'so the attached set is unknown$')"

reset
echo malformed >"$fixture/devices-mode"
smoke
check "a malformed device listing fails the gate" 1 "$status"
check "the malformed listing is reported" 1 \
    "$(reported '^FAIL adb devices listed a malformed entry')"

reset
echo headerless >"$fixture/devices-mode"
smoke
check "a device listing without its header fails the gate" 1 "$status"
check "the missing header is reported" 1 "$(reported 'printed no device list header')"

reset
rm "$avd_home/$avd.ini"
smoke
check "a missing AVD is created from the qualified image" 0 "$status"
check "the AVD is created with the name and image the gate was given" "$avd $image" \
    "$(awk '$0 == "-n" { getline name } $0 == "-k" { getline package } END { print name, package }' \
        "$fixture/avdmanager-record")"

reset
fixture_avd_home=""
smoke
unset fixture_avd_home
check "SDK preferences can supply the AVD directory" 0 "$status"
check "the emulator receives the directory used for AVD creation" "$work/android-user/avd" \
    "$(cat "$fixture/emulator-avd-home")"

reset
smoke --apk "$work/apk/absent.apk"
check "an unbuilt APK fails the gate rather than skipping it" 1 "$status"
check "the absent APK is reported" 1 "$(reported 'assemble the debug APK before this gate$')"

reset
smoke --sabotage
check "an unknown option is refused" 1 "$status"
check "the unknown option is named" 1 "$(reported '^FAIL unknown option --sabotage$')"

if [ "$problems" -eq 0 ]; then
    echo "PASS $checked device gate checks behave as specified"
else
    echo "FAILURES $problems of $checked"
    exit 1
fi
