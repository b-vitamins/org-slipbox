#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Check pinned inputs and installation arguments using mock SDK tools.

set -eu

TOOLS=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
INPUTS="$TOOLS/inputs.sh"

# Leave CI_INPUTS_LIB unexported so subprocesses execute their requested action.
CI_INPUTS_LIB=1
. "$INPUTS"

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

run() {
    label=$1
    expected=$2
    shift 2
    status=0
    sh "$INPUTS" "$@" >"$work/run.log" 2>&1 || status=$?
    check "$label" "$expected" "$status"
}

emitted() {
    sed -n "s/^$1=//p" "$work/pins"
}

catalog_version() {
    grep "^$1 = " "$MODULE/gradle/libs.versions.toml" |
        head -n 1 | tr -d '" ' | cut -d= -f2
}

# Restates the ABI declaration without the reader's own parser.
declared_entries() {
    sed -n '/^val qualifiedAbis = mapOf($/,/^)$/ s/^ *"\([^"]*\)" to "\([^"]*\)",$/\1:\2/p' \
        "$MODULE/app/build.gradle.kts"
}

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

sh "$INPUTS" >"$work/pins"

check "the pinned inputs are the declared set" \
    "abis agp build_tools compile_sdk gradle gradle_sha256 jdk kotlin min_sdk ndk platform rust rust_targets system_image target_sdk" \
    "$(cut -d= -f1 <"$work/pins" | sort | tr '\n' ' ' | sed 's/ $//')"
check "every input is a step output name with a value" 0 \
    "$(grep -vc '^[a-z][a-z0-9_]*=..*$' "$work/pins" || true)"

for version in jdk agp kotlin rust ndk build-tools compile-sdk min-sdk target-sdk; do
    check "$version is the version the catalog declares" "$(catalog_version "$version")" \
        "$(emitted "$(echo "$version" | tr '-' '_')")"
done

wrapper="$MODULE/gradle/wrapper/gradle-wrapper.properties"
check "gradle is the version the wrapper downloads" \
    "$(grep '^distributionUrl=' "$wrapper" | sed 's/.*gradle-//; s/-bin\.zip$//')" \
    "$(emitted gradle)"
check "gradle_sha256 is the checksum the wrapper verifies" \
    "$(grep '^distributionSha256Sum=' "$wrapper" | cut -d= -f2)" "$(emitted gradle_sha256)"
check "gradle_sha256 is a sha256 digest" 1 \
    "$(emitted gradle_sha256 | grep -c '^[0-9a-f]\{64\}$' || true)"

check "the platform is minor 0 of the compiled API" "android-$(emitted compile_sdk).0" \
    "$(emitted platform)"
check "abis are the qualified ABIs of the build script" \
    "$(declared_entries | cut -d: -f1 | tr '\n' ' ' | sed 's/ $//')" \
    "$(emitted abis)"
check "rust_targets are the targets those ABIs map to" \
    "$(declared_entries | cut -d: -f2 | tr '\n' ' ' | sed 's/ $//')" \
    "$(emitted rust_targets)"
case $(uname -m) in
arm64 | aarch64) accelerated=arm64-v8a ;;
x86_64 | amd64) accelerated=x86_64 ;;
*) accelerated="" ;;
esac
check "the system image is the compiled platform for this host's accelerated ABI" \
    "system-images;$(emitted platform);google_apis;$accelerated" \
    "$(emitted system_image)"
check "the image ABI is one the build script qualifies" 1 \
    "$(emitted abis | tr ' ' '\n' | grep -cx "$accelerated" || true)"
check "one Rust target is declared per qualified ABI" \
    "$(emitted abis | wc -w | tr -d ' ')" "$(emitted rust_targets | wc -w | tr -d ' ')"

status=0
(pin no-such-input >/dev/null 2>"$work/pin.err") || status=$?
check "an undeclared input fails the read" 1 "$status"
check "the undeclared input is named" 1 "$(grep -c 'declares no no-such-input' "$work/pin.err" || true)"

mkdir "$work/unhosted"
printf '#!/bin/sh\necho riscv64\n' >"$work/unhosted/uname"
chmod +x "$work/unhosted/uname"
status=0
env PATH="$work/unhosted:$PATH" sh "$INPUTS" >"$work/run.log" 2>&1 || status=$?
check "a host with no accelerated emulator fails the read" 1 "$status"
check "the unhosted architecture is named" 1 \
    "$(grep -c 'riscv64 hosts no accelerated Android emulator' "$work/run.log" || true)"

echo 'kept' >"$work/github-output"
status=0
env PATH="$work/unhosted:$PATH" sh "$INPUTS" --github-output "$work/github-output" \
    >"$work/run.log" 2>&1 || status=$?
check "a failed input read fails the step-output command" 1 "$status"
check "a failed input read leaves step outputs unchanged" kept "$(cat "$work/github-output")"
run "the pinned inputs append to a step output file" 0 --github-output "$work/github-output"
check "the step output file keeps what it already held" kept "$(head -n 1 "$work/github-output")"
check "the appended lines are the emitted lines" "$(cat "$work/pins")" \
    "$(tail -n +2 "$work/github-output")"
check "the appended lines are also reported" "$(cat "$work/pins")" "$(cat "$work/run.log")"

image=$(emitted system_image)
sh "$INPUTS" --packages emulator "$image" >"$work/packages"
check "the packages are the pinned SDK inputs and the extras given" \
    "platforms;$(emitted platform) build-tools;$(emitted build_tools) ndk;$(emitted ndk) platform-tools emulator $image" \
    "$(tr '\n' ' ' <"$work/packages" | sed 's/ $//')"
run "an empty package name is refused" 1 --packages ""
run "the pinned inputs take no package argument" 1 emulator
run "--print applies to the install alone" 1 --print
run "a step output file is not a package list" 1 --packages --github-output "$work/github-output"
run "an unknown option is refused" 1 --sdk

mkdir -p "$work/sdk/cmdline-tools/latest/bin" "$work/toolbox" "$work/empty"
for tool in sh dirname sed awk tr cut tee yes head; do
    ln -s "$(command -v "$tool")" "$work/toolbox/$tool"
done
SDK_RECORD="$work/sdk-record"
RUSTUP_RECORD="$work/rustup-record"
export SDK_RECORD RUSTUP_RECORD

cat >"$work/sdk/cmdline-tools/latest/bin/sdkmanager" <<'MOCK'
#!/bin/sh
set -eu
: >"$SDK_RECORD"
for argument in "$@"; do
    echo "$argument" >>"$SDK_RECORD"
done
head -n 3 >"$SDK_RECORD.answers"
MOCK
cat >"$work/toolbox/rustup" <<'MOCK'
#!/bin/sh
set -eu
: >"$RUSTUP_RECORD"
for argument in "$@"; do
    echo "$argument" >>"$RUSTUP_RECORD"
done
MOCK
chmod +x "$work/sdk/cmdline-tools/latest/bin/sdkmanager" "$work/toolbox/rustup"

status=0
env PATH="$work/toolbox" ANDROID_HOME="$work/sdk" sh "$INPUTS" --install --print emulator \
    >"$work/plan" 2>&1 || status=$?
check "the install plan is reported" 0 "$status"
check "the plan accepts the licences of the packages it names" \
    "yes | $work/sdk/cmdline-tools/latest/bin/sdkmanager --sdk_root=$work/sdk platforms;$(emitted platform) build-tools;$(emitted build_tools) ndk;$(emitted ndk) platform-tools emulator" \
    "$(head -n 1 "$work/plan")"
# One flag per target: words after a single --target would name toolchains.
target_flags=$(emitted rust_targets | tr ' ' '\n' | sed 's/^/--target /' |
    tr '\n' ' ' | sed 's/ $//')
check "the plan installs the pinned toolchain with every target" \
    "rustup toolchain install $(emitted rust) --profile minimal $target_flags" \
    "$(tail -n 1 "$work/plan")"
check "the plan runs no installer" "0 0" \
    "$(ls "$SDK_RECORD" 2>/dev/null | wc -l | tr -d ' ') $(ls "$RUSTUP_RECORD" 2>/dev/null | wc -l | tr -d ' ')"

status=0
env PATH="$work/toolbox" ANDROID_HOME="$work/sdk" sh "$INPUTS" --install emulator \
    >"$work/install.log" 2>&1 || status=$?
check "the install runs both provisioning commands" 0 "$status"
check "the SDK installs exactly the packages the plan named" \
    "--sdk_root=$work/sdk platforms;$(emitted platform) build-tools;$(emitted build_tools) ndk;$(emitted ndk) platform-tools emulator" \
    "$(tr '\n' ' ' <"$SDK_RECORD" | sed 's/ $//')"
check "the SDK is answered rather than left prompting" "y y y" \
    "$(tr '\n' ' ' <"$SDK_RECORD.answers" | sed 's/ $//')"
check "the toolchain install carries the pin and the targets" \
    "toolchain install $(emitted rust) --profile minimal $target_flags" \
    "$(tr '\n' ' ' <"$RUSTUP_RECORD" | sed 's/ $//')"

status=0
env -u ANDROID_HOME -u ANDROID_SDK_ROOT PATH="$work/toolbox" sh "$INPUTS" --install \
    >"$work/run.log" 2>&1 || status=$?
check "an install without an SDK location fails" 1 "$status"
check "the missing SDK location is named" 1 "$(grep -c 'ANDROID_HOME is not set' "$work/run.log" || true)"

status=0
env PATH="$work/toolbox" ANDROID_HOME="$work/empty" sh "$INPUTS" --install \
    >"$work/run.log" 2>&1 || status=$?
check "an install without the command-line tools fails" 1 "$status"
check "the SDK without command-line tools is named" 1 \
    "$(grep -c "$work/empty carries no command-line tools" "$work/run.log" || true)"

mkdir -p "$work/versioned/cmdline-tools/12.0/bin"
cp "$work/sdk/cmdline-tools/latest/bin/sdkmanager" "$work/versioned/cmdline-tools/12.0/bin/"
status=0
env PATH="$work/toolbox" ANDROID_HOME="$work/versioned" sh "$INPUTS" --install --print \
    >"$work/versioned-plan" 2>&1 || status=$?
check "a versioned command-line tools directory is accepted" 0 "$status"
check "the versioned tools install the same packages" \
    "yes | $work/versioned/cmdline-tools/12.0/bin/sdkmanager --sdk_root=$work/versioned platforms;$(emitted platform) build-tools;$(emitted build_tools) ndk;$(emitted ndk) platform-tools" \
    "$(head -n 1 "$work/versioned-plan")"

rm "$work/toolbox/rustup"
status=0
env PATH="$work/toolbox" ANDROID_HOME="$work/sdk" sh "$INPUTS" --install \
    >"$work/run.log" 2>&1 || status=$?
check "an install without rustup fails" 1 "$status"
check "the absent rustup is named" 1 "$(grep -c 'rustup is not on PATH' "$work/run.log" || true)"

if [ "$problems" -eq 0 ]; then
    echo "PASS $checked declared input checks behave as specified"
else
    echo "FAILURES $problems of $checked"
    exit 1
fi
