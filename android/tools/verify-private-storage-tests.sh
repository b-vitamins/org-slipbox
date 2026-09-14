#!/bin/sh
# Copyright (C) 2026 Ayan Das
# SPDX-License-Identifier: GPL-3.0-or-later

# Exercises tools/verify-private-storage.sh: its rule evaluator and payload census
# against synthetic input, and the verifier itself against control APKs linked
# from the packaged rule sources with one policy defect each. Every APK and
# resource this script reads is copied first; nothing it is given is modified.
#
# Usage: tools/verify-private-storage-tests.sh [APK...]
#
# Defaults to the debug and unsigned-release outputs of the last assembly, which
# must exist: an unbuilt tree is a failure here, not a skip.

set -eu

TOOLS=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

# Not exported: the verifier this script also runs as a subprocess must run its
# checks rather than only define them.
VERIFY_PRIVATE_STORAGE_LIB=1
. "$TOOLS/verify-private-storage.sh"

RULES_SOURCE="$MODULE/app/src/main/res/xml"

# The attributes a control manifest carries when nothing is mutated.
INTACT_FLAGS='android:allowBackup="false"'
INTACT_LEGACY='android:fullBackupContent="@xml/backup_rules"'
INTACT_MODERN='android:dataExtractionRules="@xml/data_extraction_rules"'

# The census scope of tools/fixtures/private-census.txt, as the fields of that
# scope with each length written out: 15, 14, 16 and 19 characters, in octal. The
# expected inventory below is derived from these rather than read from the fixture
# or from the Kotlin that generated it.
CENSUS_SOURCE='\0017census-source-1'
CENSUS_PROVIDER='\0016census.example'
CENSUS_ACCOUNT='\0020census-account-1'
CENSUS_CREDENTIAL='\0023census-credential-1'

# The name the platform gives this package's default preference file.
PREFERENCES=io.github.b_vitamins.slipbox_preferences.xml

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
    sh "$TOOLS/verify-private-storage.sh" "$@" >"$work/verifier.log" 2>&1 || status=$?
    check "$label" "$expected" "$status"
}

# Checks that the last verifier run reported $2 once, for the reason $1.
reported() {
    check "$1" 1 "$(grep -c "$2" "$work/verifier.log" || true)"
}

# Writes a control manifest to $1 with the application attributes $2..; each
# argument is one attribute, so one mutation is one changed argument.
write_manifest() {
    file=$1
    shift
    attributes=""
    for attribute in "$@"; do
        attributes="$attributes $attribute"
    done
    cat >"$file" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="io.github.b_vitamins.slipbox.control">
    <uses-sdk android:minSdkVersion="$min_api" android:targetSdkVersion="$target_api" />
    <application$attributes />
</manifest>
EOF
}

# SHA-256 over the label $1 and the length-prefixed fields $2.., which is how
# VaultScope documents a scope digest: the label, a zero byte, then each field.
census_digest() {
    label=$1
    shift
    fields=""
    for field in "$@"; do
        fields="$fields$field"
    done
    printf '%b' "$label\0000$fields" | digest_of_input
}

# Copies the packaged rule sources into the fresh resource directory $1.
copy_rules() {
    mkdir -p "$1/xml"
    cp "$RULES_SOURCE/backup_rules.xml" "$RULES_SOURCE/data_extraction_rules.xml" "$1/xml/"
}

# Links the manifest $1 and the resource directory $2 into the APK $3, the way
# the build packages them, so the verifier reads compiled resources here too.
link_control() {
    rm -f "$work/compiled.zip"
    "$aapt2" compile --dir "$2" -o "$work/compiled.zip" >"$work/aapt2.log" 2>&1 ||
        abort "aapt2 could not compile $2: $(tr '\n' ' ' <"$work/aapt2.log")"
    "$aapt2" link -o "$3" -I "$platform_jar" --manifest "$1" "$work/compiled.zip" \
        >"$work/aapt2.log" 2>&1 ||
        abort "aapt2 could not link $1: $(tr '\n' ' ' <"$work/aapt2.log")"
}

# Builds the control named $1 from the attributes in $2 and the rule mutation
# program $3 applied through `mutate`, and prints the APK it linked.
control_apk() {
    name=$1
    directory="$work/control-$name"
    mkdir "$directory"
    copy_rules "$directory/res"
    write_manifest "$directory/AndroidManifest.xml" $2
    [ -z "$3" ] || mutate "$directory/res/xml" "$3"
    link_control "$directory/AndroidManifest.xml" "$directory/res" "$directory/control.apk"
    echo "$directory/control.apk"
}

# Applies the named mutation $2 to the rule sources in $1.
mutate() {
    xml=$1
    case $2 in
    drop-cloud-root)
        awk '
            !dropped && /<exclude domain="root"/ { dropped = 1; next }
            { print }
        ' "$xml/data_extraction_rules.xml" >"$xml/rules.new"
        mv "$xml/rules.new" "$xml/data_extraction_rules.xml"
        ;;
    drop-legacy-device-root)
        awk '
            !dropped && /<exclude domain="device_root"/ { dropped = 1; next }
            { print }
        ' "$xml/backup_rules.xml" >"$xml/rules.new"
        mv "$xml/rules.new" "$xml/backup_rules.xml"
        ;;
    drop-device-transfer)
        awk '
            /<device-transfer>/ { inside = 1 }
            inside && /<\/device-transfer>/ { inside = 0; next }
            !inside { print }
        ' "$xml/data_extraction_rules.xml" >"$xml/rules.new"
        mv "$xml/rules.new" "$xml/data_extraction_rules.xml"
        ;;
    include-files)
        awk '
            { print }
            /<cloud-backup>/ { print "        <include domain=\"file\" path=\".\" />" }
        ' "$xml/data_extraction_rules.xml" >"$xml/rules.new"
        mv "$xml/rules.new" "$xml/data_extraction_rules.xml"
        ;;
    open-cross-platform | open-unknown)
        if [ "$2" = open-cross-platform ]; then
            opened=cross-platform-transfer
        else
            opened=partial-transfer
        fi
        awk -v opened="$opened" '
            { print }
            /<\/device-transfer>/ && !added {
                added = 1
                print "    <" opened ">"
                print "        <exclude domain=\"root\" path=\".\" />"
                print "    </" opened ">"
            }
        ' "$xml/data_extraction_rules.xml" >"$xml/rules.new"
        mv "$xml/rules.new" "$xml/data_extraction_rules.xml"
        ;;
    widen-rule)
        awk '
            !added && /<exclude domain="external"/ {
                added = 1
                sub(/\/>/, "requireFlags=\"clientSideEncryption\" />")
            }
            { print }
        ' "$xml/data_extraction_rules.xml" >"$xml/rules.new"
        mv "$xml/rules.new" "$xml/data_extraction_rules.xml"
        ;;
    *) abort "unknown mutation $2" ;;
    esac
}

if [ $# -eq 0 ]; then
    set -- "$MODULE/app/build/outputs/apk/debug/app-debug.apk" \
        "$MODULE/app/build/outputs/apk/release/app-release-unsigned.apk"
fi
for apk in "$@"; do
    [ -f "$apk" ] || abort "$apk does not exist; assemble the APKs before running these tests"
done

compile_api=$(pin compile-sdk)
platform_jar=""
for candidate in "$sdk/platforms/android-$compile_api"*/android.jar; do
    [ -f "$candidate" ] || continue
    [ -z "$platform_jar" ] || abort "more than one android-$compile_api platform is installed"
    platform_jar=$candidate
done
[ -n "$platform_jar" ] || abort "no android-$compile_api platform is installed under $sdk"

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT INT TERM

scope_digest=$(census_digest slipbox.vault.scope.1 \
    "$CENSUS_SOURCE" "$CENSUS_PROVIDER" "$CENSUS_ACCOUNT" "$CENSUS_CREDENTIAL")
account_digest=$(census_digest slipbox.vault.account.1 \
    "$CENSUS_SOURCE" "$CENSUS_PROVIDER" "$CENSUS_ACCOUNT")

# Read once, compared once more at the end: no input may end this run altered.
inputs="$RULES_SOURCE/backup_rules.xml $RULES_SOURCE/data_extraction_rules.xml $CENSUS_FIXTURE"
originals="$work/original-digests"
: >"$originals"
for apk in "$@"; do
    digest "$apk" >>"$originals"
done
for input in $inputs; do
    digest "$input" >>"$originals"
done

echo "EVALUATOR the verdicts the packaged rules produce"

failures=0
inspect_evaluator >"$work/report" 2>&1
check "the evaluator's own controls pass" 0 "$failures"
check "the control run reports every answer among its verdicts" 1 \
    "$(grep -c 'each answer among them' "$work/report" || true)"

cat >"$work/whole-domain" <<EOF
legacy|exclude|root|.
EOF
check "a dot exclusion covers a whole domain" excluded \
    "$(verdict_for legacy root files/sources.json "$work/whole-domain")"
check "an exclusion of one domain leaves another eligible" eligible \
    "$(verdict_for legacy file sources.json "$work/whole-domain")"
check "an exclusion in one mode leaves another mode eligible" eligible \
    "$(verdict_for cloud-backup root files/sources.json "$work/whole-domain")"
check "the platform's own exclusions answer before any rule" intrinsic \
    "$(verdict_for cloud-backup root "$PRIVATE_ROOT/vault/abc/credential.bin" "$work/whole-domain")"
check "a name the private root only prefixes is not one of them" eligible \
    "$(verdict_for cloud-backup root "${PRIVATE_ROOT}s/vault/abc" "$work/whole-domain")"
check "a cache name outside the app's own domains is not one either" eligible \
    "$(verdict_for cloud-backup file cache/scratch.bin "$work/whole-domain")"

cat >"$work/subtree" <<EOF
cloud-backup|exclude|file|$PRIVATE_ROOT/vault/
EOF
check "a rule path keeps its bound with a trailing separator" excluded \
    "$(verdict_for cloud-backup file "$PRIVATE_ROOT/vault/abc/credential.bin" "$work/subtree")"
check "a sibling of an excluded subtree stays eligible" eligible \
    "$(verdict_for cloud-backup file "$PRIVATE_ROOT/vaultkeys/abc" "$work/subtree")"
check "an empty rule set leaves an entry eligible" eligible \
    "$(: >"$work/empty" && verdict_for cloud-backup file anything "$work/empty")"

echo "CENSUS the inventory the verifier places and evaluates"

census="$work/census"
build_payload "$work/payload" >"$census"
entries=$(grep -c . "$census" || true)

cat >"$work/expected-census" <<EOF
root|$PRIVATE_ROOT/vault/$scope_digest/credential.bin|policy
root|$PRIVATE_ROOT/checkout/$account_digest/inbox.org|policy
root|$PRIVATE_ROOT/index/$account_digest/index.db|policy
root|$PRIVATE_ROOT/index/$account_digest/index.db-wal|policy
root|$PRIVATE_ROOT/index/$account_digest/index.db-shm|policy
root|$PRIVATE_ROOT/index/$account_digest/index.db-journal|policy
root|$PRIVATE_ROOT/assets/$account_digest/diagram.png|policy
root|$PRIVATE_ROOT/reading/$account_digest/positions.json|policy
device_root|$PRIVATE_ROOT/vault/$scope_digest/credential.bin|policy
root|files/sources.json|platform
root|cache/index-scratch.bin|platform
file|sources.json|platform
database|index.db|platform
database|index.db-wal|platform
database|index.db-shm|platform
database|index.db-journal|platform
sharedpref|$PREFERENCES|platform
external|assets/diagram.png|platform
device_file|positions.json|platform
device_database|index.db|platform
device_sharedpref|$PREFERENCES|platform
EOF
check "the census the verifier places is the expected inventory" \
    "$(cat "$work/expected-census")" "$(cut -d'|' -f1-3 "$census")"
check "every census entry says what it is" "$entries" \
    "$(awk -F'|' 'NF == 4 && $4 != "" { said++ } END { print said + 0 }' "$census")"
check "the census names every store the policy declares" "$(declared_stores | cut -d: -f1 | sort)" \
    "$(awk -F'|' -v root="$PRIVATE_ROOT/" '
        $1 == "root" && $3 == "policy" && index($2, root) == 1 {
            split($2, part, "/")
            print part[2]
        }
    ' "$census" | sort -u)"
check "the payload writes one file per census entry" "$entries" \
    "$(find "$work/payload" -type f | wc -l | tr -d ' ')"
check "no payload file is empty" 0 "$(find "$work/payload" -type f -empty | wc -l | tr -d ' ')"
check "the census reaches every domain the rules must exclude" \
    "$(printf '%s\n' $SENSITIVE_DOMAINS | sort)" "$(cut -d'|' -f1 "$census" | sort -u)"

# A fixture the verifier cannot account for ends the run rather than shrinking it.
refuses_census() {
    printf '%s\n' "$2" >"$work/bad-census"
    status=0
    (
        CENSUS_FIXTURE="$work/bad-census"
        build_payload "$work/bad-payload" >/dev/null
    ) >"$work/bad.log" 2>&1 || status=$?
    check "$1" 1 "$status"
}

refuses_census "a census with no entry is refused" "# comment only"
refuses_census "a census entry with no description is refused" "root|files/sources.json|policy|"
refuses_census "a census entry in an unknown domain is refused" "nowhere|sources.json|policy|a file"
refuses_census "a census entry of unknown origin is refused" "root|files/sources.json|guess|a file"
refuses_census "an absolute census path is refused" "root|/etc/hosts|platform|another app's file"
refuses_census "a census path that climbs is refused" "root|files/../../x|platform|an escape"

# Every mode of a listing that excludes what a census carries, for the checks below.
cat >"$work/all-excluded" <<EOF
legacy|exclude|root|.
cloud-backup|exclude|root|.
device-transfer|exclude|root|.
EOF
kept_census=$census
payload_root="$work/payload"

grep "^root|files/sources.json|" "$census" >"$work/one-entry"
rm "$work/payload/root/files/sources.json"
census="$work/one-entry"
failures=0
inspect_census "$work/all-excluded" control >"$work/report" 2>&1
check "a census entry with no file behind it fails" 1 "$failures"
check "the entry no file backs is named" 1 \
    "$(grep -c 'which no payload file holds' "$work/report" || true)"

: >"$work/no-entry"
census="$work/no-entry"
failures=0
inspect_census "$work/all-excluded" control >"$work/report" 2>&1
# An empty census is a failure of its own, and excludes nothing by rule either.
check "an empty census fails rather than passing" 2 "$failures"
check "the empty census is named" 1 "$(grep -c 'given an empty census' "$work/report" || true)"

grep "^root|$PRIVATE_ROOT/" "$kept_census" >"$work/intrinsic-only"
census="$work/intrinsic-only"
failures=0
inspect_census "$work/all-excluded" control >"$work/report" 2>&1
check "a census the platform alone excludes fails" 1 "$failures"
check "the rules it would not have checked are named" 1 \
    "$(grep -c 'excludes no census entry by rule' "$work/report" || true)"
census=$kept_census

echo "RULES the listing the compiled control resources produce"

intact=$(control_apk intact "$INTACT_FLAGS $INTACT_LEGACY $INTACT_MODERN" "")
dump_xmltree "$intact" res/xml/backup_rules.xml >"$work/legacy-tree"
dump_xmltree "$intact" res/xml/data_extraction_rules.xml >"$work/modern-tree"
rule_listing "$work/legacy-tree" >"$work/legacy-rules"
rule_listing "$work/modern-tree" >"$work/modern-rules"
check "every legacy rule is an exclusion in the legacy mode" \
    "$(wc -l <"$work/legacy-rules" | tr -d ' ')" \
    "$(grep -c '^legacy|exclude|' "$work/legacy-rules" || true)"
check "the modern rules carry both modes and nothing outside one" \
    "cloud-backup device-transfer" \
    "$(cut -d'|' -f1 "$work/modern-rules" | sort -u | tr '\n' ' ' | sed 's/ $//')"
check "each mode excludes every sensitive domain" \
    "$(echo "$SENSITIVE_DOMAINS" | tr ' \n' '\n\n' | grep -c .)" \
    "$(grep -c '^cloud-backup|exclude|' "$work/modern-rules" || true)"

echo "CONTROLS the policy defects a linked APK must fail on"

verify "an intact control passes" 0 "$intact"
reported "the intact control reports its own rule digest" "^POLICY all 1 APKs"
reported "the intact control reports the census it placed" "^CENSUS $entries entries"
reported "the intact control excludes some census entries by rule" \
    "excluded by rule and .* by the platform itself"

verify "a missing input fails rather than skipping" 1 "$work/absent.apk"
reported "the missing input is named" "does not exist"

backup_on=$(control_apk backup-on \
    "android:allowBackup=\"true\" $INTACT_LEGACY $INTACT_MODERN" "")
verify "a control that allows backup is rejected" 1 "$backup_on"
reported "the permissive flag is named" 'android:allowBackup=true, not false'

wrong_reference=$(control_apk wrong-reference \
    "$INTACT_FLAGS android:fullBackupContent=\"@xml/data_extraction_rules\" $INTACT_MODERN" "")
verify "a control referencing the wrong resource is rejected" 1 "$wrong_reference"
reported "the wrong reference is named" \
    'points android:fullBackupContent at xml/data_extraction_rules'

no_modern=$(control_apk no-modern "$INTACT_FLAGS $INTACT_LEGACY" "")
verify "a control with no modern rule attribute is rejected" 1 "$no_modern"
reported "the absent attribute is named" \
    'names no compiled resource in android:dataExtractionRules'
reported "the modes that attribute would carry are named" \
    'declares no cloud-backup rule at all'

no_cloud_root=$(control_apk no-cloud-root "$INTACT_FLAGS $INTACT_LEGACY $INTACT_MODERN" \
    drop-cloud-root)
verify "a control missing one cloud exclusion is rejected" 1 "$no_cloud_root"
reported "the missing exclusion is named" \
    'does not exclude the whole root domain in cloud-backup'
# The private root stays excluded by the platform itself, so what that exclusion
# was carrying is the configuration beside it, and that is what is reported.
reported "the entry it would expose is named" \
    'leaves root:files/sources.json .* eligible in cloud-backup'
check "the record the platform holds back is not called exposed" 0 \
    "$(grep -c "leaves root:$PRIVATE_ROOT/" "$work/verifier.log" || true)"

no_legacy_device_root=$(control_apk no-legacy-device-root \
    "$INTACT_FLAGS $INTACT_LEGACY $INTACT_MODERN" drop-legacy-device-root)
verify "a control missing one legacy exclusion is rejected" 1 "$no_legacy_device_root"
reported "the missing legacy exclusion is named" \
    'does not exclude the whole device_root domain in legacy'

no_transfer=$(control_apk no-transfer "$INTACT_FLAGS $INTACT_LEGACY $INTACT_MODERN" \
    drop-device-transfer)
verify "a control omitting device transfer is rejected" 1 "$no_transfer"
reported "the omitted mode is named" 'declares no device-transfer rule at all'

broad_include=$(control_apk broad-include "$INTACT_FLAGS $INTACT_LEGACY $INTACT_MODERN" \
    include-files)
verify "a control including a whole domain is rejected" 1 "$broad_include"
reported "the inclusion is named" 'includes file:\. in cloud-backup'

cross_platform=$(control_apk cross-platform "$INTACT_FLAGS $INTACT_LEGACY $INTACT_MODERN" \
    open-cross-platform)
verify "a control opting into cross-platform transfer is rejected" 1 "$cross_platform"
reported "the unauthorized section is named" \
    'opts into the cross-platform-transfer section, which this app does not authorize'
check "the unauthorized section is not read as the mode before it" 0 \
    "$(grep -c 'declares no device-transfer rule' "$work/verifier.log" || true)"

unknown_section=$(control_apk unknown-section "$INTACT_FLAGS $INTACT_LEGACY $INTACT_MODERN" \
    open-unknown)
verify "a control packaging a section this policy does not know is rejected" 1 "$unknown_section"
reported "the unrecognized section is named" \
    'packages the unrecognized rule section <partial-transfer>'

widened=$(control_apk widened "$INTACT_FLAGS $INTACT_LEGACY $INTACT_MODERN" widen-rule)
verify "a control conditioning an exclusion on a flag is rejected" 1 "$widened"
reported "the attribute this policy does not declare is named" \
    'declares requireFlags on <exclude>'

echo "APKS the built outputs, unmodified"

verify "every built APK passes" 0 "$@"
reported "the built APKs are reported as one policy" \
    "^POLICY all $# APKs package the same rules"

check "every input this run read is byte-identical afterwards" "$(cat "$originals")" \
    "$(for apk in "$@"; do digest "$apk"; done
    for input in $inputs; do digest "$input"; done)"

if [ "$problems" -eq 0 ]; then
    echo "PASS $checked private-storage checks behave as specified"
else
    echo "FAILURES $problems of $checked"
    exit 1
fi
