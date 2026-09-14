#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Usage: tools/ci/inputs.sh [--github-output FILE]
#        tools/ci/inputs.sh --packages [PACKAGE ...]
#        tools/ci/inputs.sh --install [--print] [PACKAGE ...]
#
# Prints pinned name=value inputs by default; --print previews installation.
# Installation requires SDK command-line tools and rustup, and accepts licences.
# Source with CI_INPUTS_LIB=1 to expose readers without running an action.

set -eu

MODULE=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
CATALOG="$MODULE/gradle/libs.versions.toml"
BUILD_SCRIPT="$MODULE/app/build.gradle.kts"
WRAPPER="$MODULE/gradle/wrapper/gradle-wrapper.properties"

# The SDK repository exposes only the current platform-tools package.
UNPINNED_PACKAGES="platform-tools"

plan=0

abort() {
    echo "FAIL $*" >&2
    exit 1
}

pin() {
    pinned=$(sed -n "s/^$1 = \"\\([^\"]*\\)\"\$/\\1/p" "$CATALOG")
    [ -n "$pinned" ] || abort "$CATALOG declares no $1"
    echo "$pinned"
}

wrapper_property() {
    value=$(sed -n "s/^$1=\\(.*\\)\$/\\1/p" "$WRAPPER")
    [ -n "$value" ] || abort "$WRAPPER declares no $1"
    echo "$value"
}

gradle_version() {
    version=$(wrapper_property distributionUrl |
        sed -n 's|.*/gradle-\([0-9][^-]*\)-[a-z]*\.zip$|\1|p')
    [ -n "$version" ] || abort "$WRAPPER names no Gradle distribution version"
    echo "$version"
}

# The SDK names minor API levels explicitly: release(37) is android-37.0.
platform_api() {
    echo "android-$(pin compile-sdk).0"
}

# Hardware acceleration requires a guest ABI matching its host architecture.
host_abi() {
    case $(uname -m) in
    arm64 | aarch64) echo arm64-v8a ;;
    x86_64 | amd64) echo x86_64 ;;
    *) abort "$(uname -m) hosts no accelerated Android emulator" ;;
    esac
}

system_image() {
    host=$(host_abi)
    declared_abi_targets | grep -q "^$host:" ||
        abort "$BUILD_SCRIPT does not qualify $host, the accelerated ABI of this host"
    echo "system-images;$(platform_api);google_apis;$host"
}

declared_abi_targets() {
    awk '
        /^val qualifiedAbis = mapOf\(/ { inside = 1 }
        inside {
            while (match($0, /"[^"]+" to "[^"]+"/)) {
                pair = substr($0, RSTART, RLENGTH)
                $0 = substr($0, RSTART + RLENGTH)
                gsub(/"/, "", pair)
                sub(/ to /, ":", pair)
                print pair
            }
            if (/\)/) { exit }
        }
    ' "$BUILD_SCRIPT"
}

words_of() {
    echo "$1" | tr '\n' ' ' | sed 's/ *$//'
}

pins() {
    abi_targets=$(declared_abi_targets)
    [ -n "$abi_targets" ] || abort "$BUILD_SCRIPT declares no qualified ABI"
    # Read before any output: a step output must not carry an empty image.
    image=$(system_image)
    echo "jdk=$(pin jdk)"
    echo "gradle=$(gradle_version)"
    echo "gradle_sha256=$(wrapper_property distributionSha256Sum)"
    echo "agp=$(pin agp)"
    echo "kotlin=$(pin kotlin)"
    echo "rust=$(pin rust)"
    echo "ndk=$(pin ndk)"
    echo "build_tools=$(pin build-tools)"
    echo "compile_sdk=$(pin compile-sdk)"
    echo "platform=$(platform_api)"
    echo "min_sdk=$(pin min-sdk)"
    echo "target_sdk=$(pin target-sdk)"
    echo "abis=$(words_of "$(echo "$abi_targets" | cut -d: -f1)")"
    echo "rust_targets=$(words_of "$(echo "$abi_targets" | cut -d: -f2)")"
    echo "system_image=$image"
}

packages() {
    echo "platforms;$(platform_api)"
    echo "build-tools;$(pin build-tools)"
    echo "ndk;$(pin ndk)"
    for unpinned in $UNPINNED_PACKAGES; do
        echo "$unpinned"
    done
    for extra in "$@"; do
        [ -n "$extra" ] || abort "an empty package name is not an SDK package"
        echo "$extra"
    done
}

# Command-line tools may be under latest or a versioned SDK directory.
sdkmanager_of() {
    if [ -x "$1/cmdline-tools/latest/bin/sdkmanager" ]; then
        echo "$1/cmdline-tools/latest/bin/sdkmanager"
        return
    fi
    for candidate in "$1"/cmdline-tools/*/bin/sdkmanager; do
        [ -x "$candidate" ] || continue
        echo "$candidate"
        return
    done
    abort "$1 carries no command-line tools to install with"
}

install_inputs() {
    sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
    [ -n "$sdk" ] || abort "ANDROID_HOME is not set"
    sdkmanager=$(sdkmanager_of "$sdk")
    command -v rustup >/dev/null || abort "rustup is not on PATH"

    set -- $(packages "$@")
    if [ "$plan" -eq 1 ]; then
        printf 'yes | %s --sdk_root=%s' "$sdkmanager" "$sdk"
        printf ' %s' "$@"
        printf '\n'
    else
        # Answer the selected packages' licence prompts.
        yes | "$sdkmanager" --sdk_root="$sdk" "$@"
    fi

    set -- rustup toolchain install "$(pin rust)" --profile minimal
    for pair in $(declared_abi_targets); do
        set -- "$@" --target "${pair#*:}"
    done
    if [ "$plan" -eq 1 ]; then
        printf '%s\n' "$*"
    else
        "$@"
    fi
}

main() {
    action=pins
    output=""
    while [ $# -gt 0 ]; do
        case $1 in
        --github-output)
            [ $# -ge 2 ] || abort "--github-output needs a file"
            output=$2
            shift 2
            ;;
        --packages)
            action=packages
            shift
            ;;
        --install)
            action=install
            shift
            ;;
        --print)
            plan=1
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

    [ "$action" = install ] || [ "$plan" -eq 0 ] ||
        abort "--print applies to --install alone"
    [ "$action" = pins ] || [ -z "$output" ] ||
        abort "--github-output applies to the pinned inputs alone"
    [ "$action" != pins ] || [ $# -eq 0 ] ||
        abort "the pinned inputs take no package argument: $1"

    case $action in
    pins)
        if [ -n "$output" ]; then
            values=$(pins)
            printf '%s\n' "$values" | tee -a "$output"
        else
            pins
        fi
        ;;
    packages) packages "$@" ;;
    install) install_inputs "$@" ;;
    esac
}

[ "${CI_INPUTS_LIB:-}" = 1 ] || main "$@"
