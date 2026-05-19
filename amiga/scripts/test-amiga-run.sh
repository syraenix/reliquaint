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

# Test 6: --dry-run with .rp9 → exit 0, prints fs-uae cmd without --floppy_drive_0
test_dry_run_rp9() {
    local f="$WORK_DIR/game.rp9"; touch "$f"
    local out
    out=$("$SCRIPT" --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *fs-uae* && "$out" != *floppy_drive_0* ]]
}

# Test 7: --model override with .adf → uses specified model config
test_dry_run_model_override() {
    local f="$WORK_DIR/game.adf"; touch "$f"
    local out
    out=$("$SCRIPT" --dry-run --model a1200 "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *a1200.fs-uae* ]]
}

run_test "no args prints usage and exits 2" test_no_args
run_test "--help prints usage and exits 0" test_help
run_test "missing file errors out" test_missing_file
run_test "unsupported extension errors out" test_bad_extension
run_test ".adf --dry-run uses default model config" test_dry_run_adf
run_test ".rp9 --dry-run hands bundle to fs-uae directly" test_dry_run_rp9
run_test "--model override picks correct config" test_dry_run_model_override

echo
echo "Summary: $PASS passed, $FAIL failed"
exit $((FAIL > 0 ? 1 : 0))
