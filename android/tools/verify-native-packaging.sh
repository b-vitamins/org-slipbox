#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Inspects the native libraries an APK packages, with the NDK and build tools
# this module pins. Every check that fails is reported and the script exits 1.
#
# Usage: tools/verify-native-packaging.sh [--artifacts DIRECTORY] APK...
#
# Requires ANDROID_HOME (or ANDROID_SDK_ROOT). Every APK must carry the engine
# for every ABI app/build.gradle.kts declares and for no other ABI; every
# packaged library must load on a kernel with 16 KB pages, match the ABI
# directory holding it, and need only a platform library available at the
# declared minimum API or a library the same APK packages for the same ABI.
#
# Sourcing this file with VERIFY_NATIVE_PACKAGING_LIB=1 defines the checks
# without running them; such a caller owns `failures` and must set `work` to a
# directory it owns. tools/verify-native-packaging-tests.sh drives them that way.

set -eu

MODULE=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
CATALOG="$MODULE/gradle/libs.versions.toml"
BUILD_SCRIPT="$MODULE/app/build.gradle.kts"

# The platform libraries this script admits and the API level each arrived at,
# per https://developer.android.com/ndk/guides/stable_apis and the NDK API
# reference. A NEEDED entry naming anything else is refused.
PLATFORM_LIBRARIES="ld-android.so:1 libc.so:1 libdl.so:1 libm.so:1
libstdc++.so:1 liblog.so:3 libz.so:3 libGLESv1_CM.so:4 libGLESv2.so:5
libjnigraphics.so:8 libEGL.so:9 libOpenSLES.so:9 libOpenMAXAL.so:14
libGLESv3.so:18 libmediandk.so:21 libandroid.so:23 libcamera2ndk.so:24
libvulkan.so:24 libaaudio.so:26 libnativewindow.so:26 libsync.so:26
libneuralnetworks.so:27 libamidi.so:29 libbinder_ndk.so:29"

# The engine links the SQLite amalgamation, so a NEEDED entry on the platform's
# own SQLite means the build fell back to a host library.
REFUSED_LIBRARIES="libsqlite3.so libsqlite.so"

ENGINE_LIBRARY="libslipbox_android.so"
JNI_SYMBOL="Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeRunFixtureProbe"

PAGE_ALIGNMENT=16384

# The loader derives no alignment bound from a RELRO end below this, so neither
# does this script.
DERIVED_ALIGNMENT_FLOOR=4096

failures=0

fail() {
    failures=$((failures + 1))
    echo "FAIL $*"
}

abort() {
    echo "FAIL $*" >&2
    exit 1
}

pin() {
    pinned=$(sed -n "s/^$1 = \"\\([^\"]*\\)\"\$/\\1/p" "$CATALOG")
    [ -n "$pinned" ] || abort "$CATALOG declares no $1"
    echo "$pinned"
}

declaration() {
    declared=$(sed -n "s/^val $1 = \"\\([^\"]*\\)\"\$/\\1/p" "$BUILD_SCRIPT")
    [ -n "$declared" ] || abort "$BUILD_SCRIPT declares no $1"
    echo "$declared"
}

# The declared ABI inventory as "abi:target" words, read from its one
# declaration rather than from whatever the APK happens to contain.
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

digest() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

machine_of() {
    case $1 in
    arm64-v8a) echo "AArch64 ELF64" ;;
    armeabi-v7a) echo "ARM ELF32" ;;
    x86) echo "Intel80386 ELF32" ;;
    x86_64) echo "AdvancedMicroDevicesX86-64 ELF64" ;;
    riscv64) echo "RISC-V ELF64" ;;
    *) echo "" ;;
    esac
}

# Program headers of type $1 in $2 as "vaddr:memsz:flags:align" words. The flags
# column holds one to three space-separated characters, so alignment is the last
# field and the flags are every field between the sizes and it.
segments() {
    "$readelf" -lW "$2" | awk -v want="$1" '
        $1 == want {
            flags = ""
            for (field = 7; field < NF; field++) { flags = flags $field }
            print $3 ":" $6 ":" flags ":" $NF
        }
    '
}

# How a 16 KB-page loader treats the RELRO segment at $1 of size $2 against the
# LOAD segments $3: `whole-load` when the RELRO covers the LOAD segment it
# starts, `prefix ALIGNMENT` when writable bytes follow it inside that segment,
# or `unsupported` when it starts no LOAD segment. A prefix caps the library's
# usable alignment at the alignment of the RELRO end address.
# https://android.googlesource.com/platform/bionic/+/android16-qpr2-release/linker/linker_phdr_16kib_compat.cpp
relro_verdict() {
    relro_start=$(($1))
    relro_size=$(($2))
    for segment in $3; do
        segment_start=${segment%%:*}
        segment_size=${segment#*:}
        segment_size=${segment_size%%:*}
        [ "$((segment_start))" -eq "$relro_start" ] || continue
        if [ "$((segment_size))" -le "$relro_size" ]; then
            echo whole-load
        else
            relro_end=$((relro_start + relro_size))
            echo "prefix $((relro_end & -relro_end))"
        fi
        return 0
    done
    echo unsupported
}

# Why the RELRO verdict $1 fails the 16 KB requirement, or nothing when it
# passes. A prefix end aligned below the loader's floor derives no bound, so the
# only rejection is a prefix that the loader can honour with 4 KB pages alone.
relro_objection() {
    case $1 in
    whole-load) ;;
    "prefix "*)
        derived=${1#prefix }
        if [ "$derived" -ge "$DERIVED_ALIGNMENT_FLOOR" ] && [ "$derived" -lt "$PAGE_ALIGNMENT" ]; then
            echo "ends its RELRO prefix on a $derived-byte boundary, leaving a writable tail in that segment"
        fi
        ;;
    *) echo "begins its RELRO at no LOAD segment, a layout this script cannot judge" ;;
    esac
}

# Whether the NEEDED entry $1 is admissible at minimum API $2 beside the
# same-ABI libraries $3: 0 admissible, 1 a refused host library, 2 a platform
# library newer than the declared minimum, 3 neither a platform library nor
# packaged in that same ABI.
dependency_verdict() {
    for refused in $REFUSED_LIBRARIES; do
        [ "$1" != "$refused" ] || return 1
    done
    for beside in $3; do
        [ "$1" != "$beside" ] || return 0
    done
    for platform in $PLATFORM_LIBRARIES; do
        [ "${platform%%:*}" = "$1" ] || continue
        [ "$2" -ge "${platform#*:}" ] || return 2
        return 0
    done
    return 3
}

# Checks the library $1 packaged for ABI $2 beside the same-ABI libraries $3.
inspect_library() {
    library=$1
    abi=$2
    siblings=$3
    label="$abi/$(basename "$library")"

    expected=$(machine_of "$abi")
    [ -n "$expected" ] || fail "$label sits in an ABI directory this script cannot check"
    header=$("$readelf" -h "$library" | tr -d ' ')
    for want in $expected; do
        echo "$header" | grep -q "$want" || fail "$label is not $want"
    done
    "$readelf" -h "$library" | grep -q "Type:[[:space:]]*DYN" ||
        fail "$label is not a shared object"

    loads=$(segments LOAD "$library")
    [ -n "$loads" ] || fail "$label carries no loadable segment"
    for load in $loads; do
        alignment=${load##*:}
        [ "$((alignment))" -ge "$PAGE_ALIGNMENT" ] ||
            fail "$label loads a segment aligned to $((alignment)), below $PAGE_ALIGNMENT"
    done

    relro=absent
    set -- $(segments GNU_RELRO "$library")
    if [ $# -eq 1 ]; then
        relro_memsz=${1#*:}
        relro=$(relro_verdict "${1%%:*}" "${relro_memsz%%:*}" "$loads")
        objection=$(relro_objection "$relro")
        [ -z "$objection" ] || fail "$label $objection"
    elif [ $# -eq 0 ]; then
        fail "$label carries no RELRO segment"
    else
        fail "$label carries $# RELRO segments; the loader relocates at most one"
    fi

    # An absent stack segment leaves the loader's read-write default; an
    # executable one is a link-line regression.
    stack=$(segments GNU_STACK "$library" | cut -d: -f3)
    case ${stack:-RW} in
    *E*) fail "$label requests an executable stack" ;;
    esac
    echo "  $label loads $(echo "$loads" | tr '\n' ' ')relro $relro, stack ${stack:-absent}"

    needed=$("$readelf" -dW "$library" | sed -n 's/.*Shared library: \[\(.*\)\]/\1/p')
    for name in $needed; do
        admission=0
        dependency_verdict "$name" "$min_api" "$siblings" || admission=$?
        case $admission in
        0) ;;
        1) fail "$label needs $name instead of the SQLite it links" ;;
        2) fail "$label needs $name, which the declared minimum API $min_api predates" ;;
        *) fail "$label needs $name, which is neither a platform library nor packaged for $abi" ;;
        esac
    done
    echo "  $label needs $(echo "$needed" | tr '\n' ' ')"

    exports=$("$readelf" --dyn-syms "$library" |
        awk '($5 == "GLOBAL" || $5 == "WEAK") && $7 != "UND" { print $8 }' | sort)
    sections=$("$readelf" -S "$library")
    echo "  $label exports $(echo "$exports" | tr '\n' ' ')"
    echo "  $label debug sections $(echo "$sections" | grep -c '\.debug_' || true)," \
        "symbol tables $(echo "$sections" | grep -c '\.symtab' || true)," \
        "sha256 $(digest "$library")"

    [ "$(basename "$library")" = "$ENGINE_LIBRARY" ] || return 0

    [ "$exports" = "$JNI_SYMBOL" ] ||
        fail "$label exports something other than the single JNI entry point"
    # Relocations the loader can resolve eagerly must be resolved before any of
    # this library runs, which is what the NDK's default link line asks for.
    "$readelf" -dW "$library" | grep -q 'BIND_NOW' ||
        fail "$label links without BIND_NOW"
    # The bundled amalgamation writes this header into every database it creates.
    "$strings" "$library" | grep -q 'SQLite format 3' ||
        fail "$label carries no bundled SQLite"
}

# Requires one usable unstripped engine per declared ABI under $1: the symbol
# evidence release tooling reads. An absent artifact is a failure, not a skip.
inspect_artifacts() {
    for pair in $abi_targets; do
        artifact="$1/${pair#*:}/$profile_directory/$ENGINE_LIBRARY"
        if [ ! -f "$artifact" ]; then
            fail "no unstripped $ENGINE_LIBRARY for ${pair%%:*} at $artifact"
            continue
        fi
        echo "ARTIFACT $artifact sha256 $(digest "$artifact")"

        # Written to a file, not a pipe: an early reader closing the pipe leaves
        # the symbol dump reporting a write error instead of its symbols.
        defined="$work/defined-symbols"
        dump=0
        "$nm" --defined-only "$artifact" >"$defined" 2>"$work/symbol-errors" || dump=$?
        [ "$dump" -eq 0 ] ||
            fail "$artifact has no readable symbol table: $(tr '\n' ' ' <"$work/symbol-errors")"
        for symbol in sqlite3_open_v2 sqlite3_libversion "$JNI_SYMBOL"; do
            grep -q " $symbol\$" "$defined" || fail "$artifact defines no $symbol"
        done
        echo "  defined symbols $(wc -l <"$defined" | tr -d ' '), symbol tables" \
            "$("$readelf" -S "$artifact" | grep -c '\.symtab' || true)"
    done
}

# Extracts the APK $1 into the fresh directory $2 and checks what it packages.
inspect_apk() {
    apk=$1
    tree=$2
    echo "APK $apk sha256 $(digest "$apk")"

    if "$zipalign" -c -P 16 4 "$apk"; then
        echo "  zipalign -c -P 16 4 accepts every entry"
    else
        fail "$apk fails zipalign -c -P 16 4"
    fi

    mkdir "$tree" || abort "$tree is not a fresh extraction directory"
    extraction=0
    unzip -q -o "$apk" 'lib/*' -d "$tree" || extraction=$?
    # 11 is "nothing matched", which the per-ABI engine checks below report
    # precisely; any other nonzero status means the input did not read.
    [ "$extraction" -eq 0 ] || [ "$extraction" -eq 11 ] ||
        abort "$apk did not extract: unzip exit $extraction"

    for abi in $declared_abis; do
        [ -f "$tree/lib/$abi/$ENGINE_LIBRARY" ] ||
            fail "$apk packages no $ENGINE_LIBRARY for the declared ABI $abi"
    done

    for directory in "$tree"/lib/*; do
        [ -d "$directory" ] || continue
        abi=$(basename "$directory")
        case " $(echo "$declared_abis" | tr '\n' ' ')" in
        *" $abi "*) ;;
        *)
            fail "$apk packages the undeclared ABI $abi"
            continue
            ;;
        esac

        siblings=""
        for entry in "$directory"/*.so; do
            [ -f "$entry" ] || continue
            siblings="$siblings $(basename "$entry")"
        done
        for entry in "$directory"/*.so; do
            [ -f "$entry" ] || continue
            inspect_library "$entry" "$abi" "$siblings"
        done

        engine="$directory/$ENGINE_LIBRARY"
        [ -f "$engine" ] || continue
        echo "$abi $(digest "$engine")" >>"$engine_digests"
    done
}

main() {
    artifact_root="$MODULE/app/build/rust-target"
    while [ $# -gt 0 ]; do
        case $1 in
        --artifacts)
            [ $# -ge 2 ] || abort "--artifacts needs a directory"
            artifact_root=$2
            shift 2
            ;;
        --)
            shift
            break
            ;;
        -*) abort "unknown option $1" ;;
        *) break ;;
        esac
    done
    [ $# -gt 0 ] || {
        echo "usage: tools/verify-native-packaging.sh [--artifacts DIRECTORY] APK..." >&2
        exit 2
    }

    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT INT TERM
    engine_digests="$work/engine-digests"
    : >"$engine_digests"

    apks=0
    for apk in "$@"; do
        [ -f "$apk" ] || abort "$apk does not exist"
        apks=$((apks + 1))
        # One extraction directory per input: two APKs sharing a basename must
        # not overlay each other or inherit libraries the other packaged.
        inspect_apk "$apk" "$work/apk-$apks"
    done

    for abi in $declared_abis; do
        carrying=$(grep -c "^$abi " "$engine_digests" || true)
        engines=$(awk -v abi="$abi" '$1 == abi { print $2 }' "$engine_digests" | sort -u)
        if [ "$carrying" -ne "$apks" ]; then
            fail "only $carrying of $apks APKs package $ENGINE_LIBRARY for $abi"
        elif [ "$(echo "$engines" | grep -c .)" -eq 1 ]; then
            echo "ENGINE $abi is identical in all $apks APKs, sha256 $engines"
        else
            fail "the $abi $ENGINE_LIBRARY differs between APKs: $(echo "$engines" | tr '\n' ' ')"
        fi
    done

    inspect_artifacts "$artifact_root"

    if [ "$failures" -eq 0 ]; then
        echo "PASS every packaged library satisfies the checks above"
    else
        echo "FAILURES $failures"
        exit 1
    fi
}

sdk=${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}
[ -n "$sdk" ] || abort "ANDROID_HOME is not set"

ndk_version=$(pin ndk)
build_tools_version=$(pin build-tools)
min_api=$(pin min-sdk)
abi_targets=$(declared_abi_targets)
[ -n "$abi_targets" ] || abort "$BUILD_SCRIPT declares no qualified ABI"
declared_abis=$(echo "$abi_targets" | cut -d: -f1)

# Cargo writes the `dev` profile into `debug`; every other profile names itself.
profile_directory=$(declaration nativeCargoProfile)
[ "$profile_directory" != dev ] || profile_directory=debug

prebuilt=""
for candidate in "$sdk/ndk/$ndk_version/toolchains/llvm/prebuilt/"*; do
    [ -d "$candidate" ] || continue
    [ -z "$prebuilt" ] || abort "NDK $ndk_version ships more than one prebuilt toolchain"
    prebuilt=$candidate
done
[ -n "$prebuilt" ] || abort "NDK $ndk_version is not installed under $sdk"

readelf="$prebuilt/bin/llvm-readelf"
nm="$prebuilt/bin/llvm-nm"
strings="$prebuilt/bin/llvm-strings"
zipalign="$sdk/build-tools/$build_tools_version/zipalign"
for tool in "$readelf" "$nm" "$strings" "$zipalign"; do
    [ -x "$tool" ] || abort "$tool is not executable"
done

[ "${VERIFY_NATIVE_PACKAGING_LIB:-}" = 1 ] || main "$@"
