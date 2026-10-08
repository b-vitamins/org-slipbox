#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

set -eu

MODULE=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
GRADLE="$MODULE/gradlew"
RUNNER_ARGUMENT="-Pandroid.testInstrumentationRunnerArguments.androidx.benchmark.suppressErrors=EMULATOR"

run_gradle() {
    "$GRADLE" --no-daemon --max-workers=2 --no-build-cache -p "$MODULE" "$@"
}

case ${1:-} in
smoke)
    run_gradle :benchmark:connectedBenchmarkAndroidTest \
        -Pandroid.testInstrumentationRunnerArguments.class=io.github.b_vitamins.slipbox.benchmark.BenchmarkSmokeTest \
        -Pandroid.testInstrumentationRunnerArguments.androidx.benchmark.dryRunMode.enable=true \
        "$RUNNER_ARGUMENT"
    ;;
profile)
    run_gradle :benchmark:connectedBenchmarkAndroidTest \
        -Pandroid.testInstrumentationRunnerArguments.class=io.github.b_vitamins.slipbox.benchmark.BaselineProfileGenerator \
        "$RUNNER_ARGUMENT"
    output="$MODULE/benchmark/build/outputs/connected_android_test_additional_output"
    baseline_count=$(find "$output" -type f \
        -name 'BaselineProfileGenerator_*-baseline-prof.txt' -print | wc -l | tr -d ' ')
    startup_count=$(find "$output" -type f \
        -name 'BaselineProfileGenerator_startup-startup-prof.txt' -print | wc -l | tr -d ' ')
    [ "$baseline_count" -eq 1 ] || {
        echo "expected one generated interaction profile, found $baseline_count" >&2
        exit 1
    }
    [ "$startup_count" -eq 1 ] || {
        echo "expected one generated startup profile, found $startup_count" >&2
        exit 1
    }
    baseline_temporary=$(mktemp "$MODULE/app/src/main/.baseline-prof.XXXXXX")
    startup_temporary=$(mktemp "$MODULE/app/src/main/.startup-prof.XXXXXX")
    trap 'rm -f "$baseline_temporary" "$startup_temporary"' EXIT HUP INT TERM
    find "$output" -type f \
        \( -name 'BaselineProfileGenerator_*-baseline-prof.txt' \
        -o -name 'BaselineProfileGenerator_startup-startup-prof.txt' \) \
        -exec cat {} + | LC_ALL=C sort -u >"$baseline_temporary"
    find "$output" -type f -name 'BaselineProfileGenerator_startup-startup-prof.txt' \
        -exec cat {} + | LC_ALL=C sort -u >"$startup_temporary"
    mv "$baseline_temporary" "$MODULE/app/src/main/baseline-prof.txt"
    mv "$startup_temporary" "$MODULE/app/src/main/startup-prof.txt"
    trap - EXIT HUP INT TERM
    "$MODULE/tools/verify-performance-profile.sh"
    ;;
measure)
    run_gradle :benchmark:connectedBenchmarkAndroidTest \
        -Pandroid.testInstrumentationRunnerArguments.class=io.github.b_vitamins.slipbox.benchmark.StartupBenchmark,io.github.b_vitamins.slipbox.benchmark.InteractionBenchmark \
        "$RUNNER_ARGUMENT"
    ;;
check)
    shift
    exec node "$MODULE/tools/check-performance-report.mjs" --reference \
        "$MODULE/benchmark/budgets.json" "$@"
    ;;
observe)
    shift
    exec node "$MODULE/tools/check-performance-report.mjs" --observe \
        "$MODULE/benchmark/budgets.json" "$@"
    ;;
*)
    echo "usage: tools/performance.sh {smoke|profile|measure|check REPORT...|observe REPORT...}" >&2
    exit 2
    ;;
esac
