#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Exercises tools/verify-native-packaging.sh: its shared segment and dependency
# checks against synthetic input, and the verifier itself against copies of built
# APKs with one engine entry removed. Every APK and artifact this script reads is
# copied first; nothing it is given is modified.
#
# Usage: tools/verify-native-packaging-tests.sh [--artifacts DIRECTORY] [APK...]
#
# Defaults to the debug and unsigned-release outputs of the last assembly, which
# must exist: an unbuilt tree is a failure here, not a skip.

set -eu

TOOLS=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# Not exported: the verifier this script also runs as a subprocess must run its
# checks rather than only define them.
VERIFY_NATIVE_PACKAGING_LIB=1
. "$TOOLS/verify-native-packaging.sh"

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

# Runs the real verifier on the APKs $3.. and checks its exit status against $2.
verify() {
    label=$1
    expected=$2
    shift 2
    status=0
    sh "$TOOLS/verify-native-packaging.sh" --artifacts "$artifact_root" "$@" \
        >"$work/verifier.log" 2>&1 || status=$?
    check "$label" "$expected" "$status"
}

# Deletes one ABI's engine entry from the APK copy $1.
remove_engine() {
    (cd "$(dirname "$1")" && zip -q -d "$(basename "$1")" "lib/$2/$ENGINE_LIBRARY")
}

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
if [ $# -eq 0 ]; then
    set -- "$MODULE/app/build/outputs/apk/debug/app-debug.apk" \
        "$MODULE/app/build/outputs/apk/release/app-release-unsigned.apk"
fi
command -v zip >/dev/null || abort "zip is required to build the negative controls"
[ -x "$prebuilt/bin/llvm-strip" ] || abort "$prebuilt/bin/llvm-strip is not executable"
for apk in "$@"; do
    [ -f "$apk" ] || abort "$apk does not exist; assemble the APKs before running these tests"
done

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

primary=$(echo "$abi_targets" | head -1)
primary_abi=${primary%%:*}
primary_target=${primary#*:}

# Read once, compared once more at the end: no input may end this run altered.
originals="$work/original-digests"
: >"$originals"
for apk in "$@"; do
    digest "$apk" >>"$originals"
done

echo "SEGMENTS the layouts the loader accepts and rejects"

# The engine's own headers: one RW LOAD its RELRO covers completely.
engine_loads="0x000000:0x77e930:RE:0x4000 0x77e930:0x4d6d0:RW:0x4000"
check "the engine's RELRO covers its whole LOAD segment" whole-load \
    "$(relro_verdict 0x77e930 0x4d6d0 "$engine_loads")"
check "a whole-LOAD RELRO raises no objection" "" \
    "$(relro_objection "$(relro_verdict 0x77e930 0x4d6d0 "$engine_loads")")"

# The graphics-path library's headers: a RELRO larger than the LOAD it protects,
# whose end alignment therefore bounds nothing.
graphics_loads="0x000000:0x5b70:RE:0x4000 0x5c40:0x390:RW:0x4000 0x9fd0:0x120:RW:0x4000"
check "a RELRO wider than its LOAD segment is a whole-LOAD case" whole-load \
    "$(relro_verdict 0x5c40 0x3c0 "$graphics_loads")"
check "the graphics-path layout raises no objection" "" \
    "$(relro_objection "$(relro_verdict 0x5c40 0x3c0 "$graphics_loads")")"

prefix_loads="0x000000:0x1000:RE:0x4000 0x1000:0x8000:RW:0x4000"
check "a RELRO prefix ending on a 16 KB boundary is a prefix of that alignment" \
    "prefix 16384" "$(relro_verdict 0x1000 0x3000 "$prefix_loads")"
check "a 16 KB-aligned prefix raises no objection" "" \
    "$(relro_objection "$(relro_verdict 0x1000 0x3000 "$prefix_loads")")"

check "a RELRO prefix ending on an 8 KB boundary is bounded at 8 KB" \
    "prefix 8192" "$(relro_verdict 0x1000 0x1000 "$prefix_loads")"
check "an 8 KB-aligned prefix leaving a writable tail is rejected" 1 \
    "$(relro_objection "$(relro_verdict 0x1000 0x1000 "$prefix_loads")" | grep -c 'writable tail' || true)"

check "a prefix ending below the loader's floor derives no bound" \
    "prefix 2048" "$(relro_verdict 0x1000 0x800 "$prefix_loads")"
check "a sub-4 KB prefix end raises no objection" "" \
    "$(relro_objection "$(relro_verdict 0x1000 0x800 "$prefix_loads")")"

check "a RELRO starting inside no LOAD segment is not judged silently" \
    unsupported "$(relro_verdict 0x9000 0x100 "$prefix_loads")"
check "an unjudgeable RELRO layout is rejected" 1 \
    "$(relro_objection "$(relro_verdict 0x9000 0x100 "$prefix_loads")" | grep -c 'no LOAD segment' || true)"

echo "DEPENDENCIES the NEEDED entries the declared minimum API admits"

admission() {
    verdict=0
    dependency_verdict "$@" || verdict=$?
    echo "$verdict"
}

check "bionic's core libraries are admissible" 0 "$(admission libc.so 23 "")"
check "the platform SQLite is refused whatever the API" 1 "$(admission libsqlite3.so 99 "")"
check "a platform library the minimum API predates is refused" 2 "$(admission libvulkan.so 23 "")"
check "a platform library the minimum API includes is admissible" 0 \
    "$(admission libandroid.so 23 "")"
check "a library packaged in the same ABI is admissible" 0 \
    "$(admission libslipbox_android.so 23 " libc++_shared.so libslipbox_android.so")"
check "a library packaged only in another ABI is refused" 3 \
    "$(admission libslipbox_android.so 23 " libc++_shared.so")"
check "an unknown library is refused rather than assumed present" 3 \
    "$(admission libinvented.so 23 "")"

echo "ARTIFACTS the unstripped engine evidence a release needs"

failures=0
inspect_artifacts "$work/absent-artifacts" >"$work/report" 2>&1
check "an absent artifact directory fails instead of passing silently" 1 "$failures"
check "the absent artifact is named" 1 \
    "$(grep -c "no unstripped $ENGINE_LIBRARY for $primary_abi" "$work/report" || true)"

failures=0
inspect_artifacts "$artifact_root" >"$work/report" 2>&1
check "the built artifact tree passes the same check" 0 "$failures"

stripped="$work/stripped-artifacts/$primary_target/$profile_directory"
mkdir -p "$stripped"
cp "$artifact_root/$primary_target/$profile_directory/$ENGINE_LIBRARY" "$stripped/"
"$prebuilt/bin/llvm-strip" --strip-all "$stripped/$ENGINE_LIBRARY"
failures=0
inspect_artifacts "$work/stripped-artifacts" >"$work/report" 2>&1
check "a stripped engine is not accepted as unstripped symbol evidence" yes \
    "$(if [ "$failures" -gt 0 ]; then echo yes; else echo no; fi)"
check "the missing SQLite symbol is named" 1 \
    "$(grep -c 'defines no sqlite3_open_v2' "$work/report" || true)"

echo "EXPORTS the closed JNI inventory the engine must carry"

dropped=$(echo "$JNI_SYMBOLS" | grep . | tail -1)
short=$(echo "$JNI_SYMBOLS" | grep . | grep -v "^$dropped\$")
invented="Java_io_github_b_1vitamins_slipbox_engine_SlipboxNativeEngine_nativeInvented"

check "the declared inventory raises no objection against itself" "" \
    "$(export_objection "$JNI_SYMBOLS" "$JNI_SYMBOLS")"
check "a list short of one symbol names the one it lacks" 1 \
    "$(export_objection "$short" "$JNI_SYMBOLS" | grep -c "missing $dropped" || true)"
check "a list carrying one symbol too many names it" 1 \
    "$(export_objection "$JNI_SYMBOLS$invented" "$JNI_SYMBOLS" | grep -c "extra $invented" || true)"
check "the single-symbol assumption no longer passes" 1 \
    "$(export_objection "$(echo "$JNI_SYMBOLS" | grep nativeRunFixtureProbe)" "$JNI_SYMBOLS" |
        grep -c 'missing .*nativeAdapterContract' || true)"

# Real libraries, linked here with the same page size and RELRO the engine uses:
# the checks above are only worth what they say about an actual ELF.
[ -x "$prebuilt/bin/clang" ] || abort "$prebuilt/bin/clang is not executable"
stub() {
    directory="$work/stub-$1"
    mkdir -p "$directory"
    : >"$directory/stub.c"
    printf '{\n  global:\n' >"$directory/stub.map"
    for symbol in $2; do
        echo "void $symbol(void) {}" >>"$directory/stub.c"
        echo "    $symbol;" >>"$directory/stub.map"
    done
    printf '  local:\n    *;\n};\n' >>"$directory/stub.map"
    "$prebuilt/bin/clang" "--target=$primary_target$min_api" -shared -fPIC \
        -o "$directory/$ENGINE_LIBRARY" "$directory/stub.c" \
        -Wl,--version-script="$directory/stub.map" \
        -Wl,-z,relro,-z,now,-z,max-page-size="$PAGE_ALIGNMENT" \
        2>"$directory/clang.log" ||
        abort "the $1 stub did not link: $(tr '\n' ' ' <"$directory/clang.log")"
    echo "$directory/$ENGINE_LIBRARY"
}

whole=$(stub inventory "$JNI_SYMBOLS")
check "a library exporting the inventory raises no objection" "" \
    "$(export_objection "$(exported_symbols "$whole")" "$JNI_SYMBOLS")"
failures=0
inspect_library "$whole" "$primary_abi" " $ENGINE_LIBRARY" >"$work/report" 2>&1
check "the library check raises no export objection against it" 0 \
    "$(grep -c 'does not export' "$work/report" || true)"

lacking=$(stub missing "$short")
failures=0
inspect_library "$lacking" "$primary_abi" " $ENGINE_LIBRARY" >"$work/report" 2>&1
check "a library short of one JNI symbol is rejected" 1 \
    "$(grep -c "missing $dropped" "$work/report" || true)"

surplus=$(stub extra "$JNI_SYMBOLS$invented")
failures=0
inspect_library "$surplus" "$primary_abi" " $ENGINE_LIBRARY" >"$work/report" 2>&1
check "a library exporting one symbol too many is rejected" 1 \
    "$(grep -c "extra $invented" "$work/report" || true)"

# The engine a release ships, read the same way.
check "the built engine exports exactly the inventory" "" \
    "$(export_objection \
        "$(exported_symbols "$artifact_root/$primary_target/$profile_directory/$ENGINE_LIBRARY")" \
        "$JNI_SYMBOLS")"

echo "APKS the engine every input must carry, on copies of the built outputs"

intact=""
index=0
for apk in "$@"; do
    index=$((index + 1))
    mkdir -p "$work/intact/$index"
    cp "$apk" "$work/intact/$index/$(basename "$apk")"
    intact="$intact $work/intact/$index/$(basename "$apk")"
done
verify "intact copies of every built APK pass" 0 $intact

for abi in $declared_abis; do
    index=0
    for apk in "$@"; do
        index=$((index + 1))
        variant="$work/no-engine-$abi-$index"
        mkdir "$variant"
        cp "$apk" "$variant/$(basename "$apk")"
        remove_engine "$variant/$(basename "$apk")" "$abi"

        arguments=""
        other=0
        for sibling in "$@"; do
            other=$((other + 1))
            if [ "$other" -eq "$index" ]; then
                arguments="$arguments $variant/$(basename "$sibling")"
            else
                arguments="$arguments $work/intact/$other/$(basename "$sibling")"
            fi
        done
        verify "APK $index without its $abi engine is rejected" 1 $arguments
        check "APK $index reports the missing $abi engine" 1 \
            "$(grep -c "packages no $ENGINE_LIBRARY for the declared ABI $abi" "$work/verifier.log" || true)"
    done
done

mkdir "$work/collide-intact" "$work/collide-stripped"
cp "$1" "$work/collide-intact/app.apk"
cp "$1" "$work/collide-stripped/app.apk"
remove_engine "$work/collide-stripped/app.apk" "$primary_abi"
verify "two inputs sharing one basename do not overlay each other" 1 \
    "$work/collide-intact/app.apk" "$work/collide-stripped/app.apk"
check "the overlaying pair reports the missing engine once" 1 \
    "$(grep -c "packages no $ENGINE_LIBRARY for the declared ABI $primary_abi" "$work/verifier.log" || true)"
verify "two copies of one APK pass while both carry the engine" 0 \
    "$work/collide-intact/app.apk" "$work/intact/1/$(basename "$1")"

check "every APK this run read is byte-identical afterwards" "$(cat "$originals")" \
    "$(for apk in "$@"; do digest "$apk"; done)"

if [ "$problems" -eq 0 ]; then
    echo "PASS $checked packaging checks behave as specified"
else
    echo "FAILURES $problems of $checked"
    exit 1
fi
