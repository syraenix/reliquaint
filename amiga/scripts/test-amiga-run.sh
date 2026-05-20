#!/bin/bash
# Minimal contract tests for amiga-run.sh.
# Usage: ./test-amiga-run.sh
# Each test prints "PASS"/"FAIL"; the script exits non-zero if any test fails.

set -uo pipefail

SCRIPT="$(cd "$(dirname "$0")" && pwd)/amiga-run.sh"
WORK_DIR="$(mktemp -d)"
trap 'rm -rf "$WORK_DIR"' EXIT

PASS=0
FAIL=0

run_test() {
    local name="$1"; shift
    if "$@"; then
        echo "PASS: $name"
        PASS=$((PASS + 1))
    else
        echo "FAIL: $name"
        FAIL=$((FAIL + 1))
    fi
}

# Test 1: no args → exit 2 with usage on stderr
test_no_args() {
    local out
    out=$("$SCRIPT" 2>&1); local ec=$?
    [[ $ec -eq 2 && "$out" == *Usage* ]]
}

# Test 2: --help → exit 0 with usage on stdout
test_help() {
    local out
    out=$("$SCRIPT" --help 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *Usage* ]]
}

# Test 3: nonexistent file → exit 2
test_missing_file() {
    local out
    out=$("$SCRIPT" "$WORK_DIR/nope.adf" 2>&1); local ec=$?
    [[ $ec -eq 2 && "$out" == *"not found"* ]]
}

# Test 4: unsupported extension → exit 2
test_bad_extension() {
    local f="$WORK_DIR/foo.zip"; touch "$f"
    local out
    out=$("$SCRIPT" "$f" 2>&1); local ec=$?
    [[ $ec -eq 2 && "$out" == *unsupported* ]]
}

# Test 5: --dry-run with .adf → exit 0, prints fs-uae cmd with --floppy_drive_0
test_dry_run_adf() {
    local f="$WORK_DIR/game.adf"; touch "$f"
    local out
    out=$("$SCRIPT" --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *fs-uae* && "$out" == *floppy_drive_0* && "$out" == *a500.fs-uae* ]]
}

# Helper: build a .rp9 (zip) fixture with the given inner files.
# Usage: make_rp9 <out.rp9> <inner-name>[:<content>] [<inner-name>[:<content>] ...]
# If content is omitted, an empty file is created.
make_rp9() {
    local out="$1"; shift
    local stage; stage="$(mktemp -d "$WORK_DIR/stage.XXXXXX")"
    local entry name content
    for entry in "$@"; do
        if [[ "$entry" == *:* ]]; then
            name="${entry%%:*}"; content="${entry#*:}"
        else
            name="$entry"; content=""
        fi
        printf '%s' "$content" > "$stage/$name"
    done
    ( cd "$stage" && python3 -m zipfile -c "$out" ./* ) >/dev/null
}

# Test 6: --dry-run with .rp9 that contains an inner .adf → unpacks and pairs
# the inner .adf with the default model config (a500), same shape as the
# raw-.adf path.
test_dry_run_rp9_with_adf() {
    local f="$WORK_DIR/game.rp9"
    make_rp9 "$f" "ziriax.adf"
    local out
    out=$("$SCRIPT" --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *fs-uae* && "$out" == *a500.fs-uae* \
        && "$out" == *floppy_drive_0*ziriax.adf* ]]
}

# Test 7: --dry-run with .rp9 that ships its own Config.fs-uae → uses that
# inner config and does NOT add --floppy_drive_0.
test_dry_run_rp9_with_inner_config() {
    local f="$WORK_DIR/game.rp9"
    make_rp9 "$f" "Config.fs-uae:[config]\namiga_model = A500\n" "game.adf"
    local out
    out=$("$SCRIPT" --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *fs-uae* && "$out" == *Config.fs-uae* \
        && "$out" != *floppy_drive_0* && "$out" != *a500.fs-uae* ]]
}

# Test 8: --dry-run with a .rp9 that has neither .adf nor .fs-uae → exit 2.
test_dry_run_rp9_empty() {
    local f="$WORK_DIR/empty.rp9"
    make_rp9 "$f" "rp9-manifest.xml:<manifest/>"
    local out
    out=$("$SCRIPT" --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 2 && "$out" == *bundle* ]]
}

# Test 9: --model override on the .rp9 unpack-to-.adf path → picks a1200.
test_dry_run_rp9_model_override() {
    local f="$WORK_DIR/game.rp9"
    make_rp9 "$f" "game.adf"
    local out
    out=$("$SCRIPT" --dry-run --model a1200 "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *a1200.fs-uae* && "$out" == *floppy_drive_0*game.adf* ]]
}

# Test 10: --model override with .adf → uses specified model config
test_dry_run_model_override() {
    local f="$WORK_DIR/game.adf"; touch "$f"
    local out
    out=$("$SCRIPT" --dry-run --model a1200 "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *a1200.fs-uae* ]]
}

# Test 11: --windowed flag with .adf → cmd ends with --fullscreen=0
test_dry_run_windowed_adf() {
    local f="$WORK_DIR/game.adf"; touch "$f"
    local out
    out=$("$SCRIPT" --windowed --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *--fullscreen=0* ]]
}

# Test 12: --windowed flag with .rp9 → cmd includes --fullscreen=0
test_dry_run_windowed_rp9() {
    local f="$WORK_DIR/game.rp9"
    make_rp9 "$f" "game.adf"
    local out
    out=$("$SCRIPT" --windowed --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *--fullscreen=0* ]]
}

run_test "no args prints usage and exits 2" test_no_args
run_test "--help prints usage and exits 0" test_help
run_test "missing file errors out" test_missing_file
run_test "unsupported extension errors out" test_bad_extension
run_test ".adf --dry-run uses default model config" test_dry_run_adf
run_test ".rp9 with inner .adf is unpacked and paired with model config" test_dry_run_rp9_with_adf
run_test ".rp9 with inner Config.fs-uae uses that inner config" test_dry_run_rp9_with_inner_config
run_test ".rp9 with neither .adf nor .fs-uae errors out" test_dry_run_rp9_empty
run_test "--model override on .rp9 unpack path picks correct config" test_dry_run_rp9_model_override
run_test "--model override picks correct config" test_dry_run_model_override
run_test "--windowed appends --fullscreen=0 (.adf)" test_dry_run_windowed_adf
run_test "--windowed appends --fullscreen=0 (.rp9)" test_dry_run_windowed_rp9

echo
echo "Summary: $PASS passed, $FAIL failed"
exit $((FAIL > 0 ? 1 : 0))
