# Amiga Collection (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an `amiga/` collection so a user can drop `.adf` or `.rp9` Amiga game files into `amiga/games/` and launch them with one generic script, following the same docs-first pattern as the existing DOS collections.

**Architecture:** Mirror the structure of `dos/<collection>/` for Amiga: a Markdown guide, FS-UAE config templates per Amiga model (A500, A1200) under `amiga/config/`, and a single generic launcher script `amiga/scripts/amiga-run.sh <file>` instead of per-game scripts. The script detects format by extension — for `.adf` it pairs the file with a model config via `--floppy_drive_0=<path>`; for `.rp9` it hands the bundle directly to FS-UAE, which reads model and config from inside the bundle. No Kickstart ROMs ship with the repo — the guide instructs users where to source them legally and the standard FS-UAE Kickstart directory layout.

**Tech Stack:** Bash, FS-UAE (apt `fs-uae` package on Debian 13+), Markdown.

**Reference roadmap:** `~/.claude/plans/now-that-we-have-serialized-nygaard.md` (Phase 1 section).

---

## Scope Check

This plan is one phase of a larger roadmap. It is deliberately self-contained — Phase 1 produces a working second collection without any of the launcher work. The `amiga-run.sh` script written here is a deliberately throwaway intermediate; Phase 2 retires it in favor of the Rust CLI engine.

---

## File Structure

**To create:**
- `amiga/amiga.md` — user-facing guide
- `amiga/scripts/amiga-run.sh` — generic launcher (bash)
- `amiga/scripts/test-amiga-run.sh` — minimal bash test harness for the launcher
- `amiga/config/a500.fs-uae` — A500 + Kickstart 1.3 FS-UAE config template
- `amiga/config/a1200.fs-uae` — A1200 + Kickstart 3.1 FS-UAE config template
- `amiga/games/.gitkeep` — preserve empty drop directory in git

**To modify:**
- `.gitignore` — add `*.adf`, `*.rp9`, `*.hdf` (gitignored anywhere in the tree, like the existing `*.exe` rule)
- `README.md` — add Amiga collection to the Game Guides list
- `docs/prerequisites.md` — add FS-UAE install section

---

## Task 1: Add FS-UAE to the prerequisites guide

**Files:**
- Modify: `docs/prerequisites.md` — append a new section

- [ ] **Step 1: Append FS-UAE section to docs/prerequisites.md**

After the existing `## innoextract` section (the file ends with the `$ sudo apt install innoextract` code block), append the following content. Outer block uses tildes so the inner triple-backtick fence stays intact — when writing the file, use literal triple-backticks for the bash block.

~~~markdown

## FS-UAE

[FS-UAE](https://fs-uae.net/) is a cross-platform Amiga emulator based on WinUAE. It is used for the Amiga collection.

FS-UAE can be installed by running the following command:

```bash
$ sudo apt install fs-uae
```

FS-UAE itself does not include Kickstart ROMs (the Amiga's firmware), which are required to boot most Amiga software. You must supply your own legally obtained Kickstart ROMs. The Amiga collection guide explains where FS-UAE expects them.
~~~

- [ ] **Step 2: Verify markdown renders sensibly**

Run: `cat docs/prerequisites.md`
Expected: file ends with the new FS-UAE section, no broken fences, no merged-with-previous-section paragraphs.

- [ ] **Step 3: Commit**

```bash
git add docs/prerequisites.md
git commit -m "docs(prereqs): add FS-UAE install section for Amiga collection"
```

---

## Task 2: Extend .gitignore for Amiga formats

**Files:**
- Modify: `.gitignore`

- [ ] **Step 1: Append Amiga format patterns**

The existing `.gitignore` contains a single line: `*.exe`. Replace its contents with:

```
*.exe
*.adf
*.rp9
*.hdf
```

- [ ] **Step 2: Verify with a temp file**

Run:
```bash
touch ./fake-game.adf
git status --short | grep fake-game.adf || echo "ignored as expected"
rm ./fake-game.adf
```

Expected: the `echo` runs and prints `ignored as expected`. If `fake-game.adf` shows up in `git status`, the gitignore rule is wrong — fix it before proceeding.

- [ ] **Step 3: Commit**

```bash
git add .gitignore
git commit -m "chore(gitignore): ignore Amiga floppy and bundle formats"
```

---

## Task 3: Scaffold the amiga/ directory structure

**Files:**
- Create: `amiga/games/.gitkeep` (empty file)
- Create: `amiga/config/` (directory; created implicitly when config files are added in Tasks 4–5)
- Create: `amiga/scripts/` (directory; created implicitly when scripts are added in later tasks)

- [ ] **Step 1: Create the games drop directory and placeholder**

Run:
```bash
mkdir -p amiga/games
touch amiga/games/.gitkeep
```

- [ ] **Step 2: Verify**

Run: `ls -la amiga/games/`
Expected: directory exists, contains only `.gitkeep`.

- [ ] **Step 3: Commit**

```bash
git add amiga/games/.gitkeep
git commit -m "feat(amiga): scaffold amiga/games drop directory"
```

---

## Task 4: Write the A500 FS-UAE config template

**Files:**
- Create: `amiga/config/a500.fs-uae`

- [ ] **Step 1: Create the config file**

Write `amiga/config/a500.fs-uae` with this content:

```ini
# FS-UAE configuration: Amiga 500 (Kickstart 1.3 / OCS).
# Used for most late-80s Amiga games. Pair with an .adf via:
#   fs-uae a500.fs-uae --floppy_drive_0=/path/to/game.adf
#
# Kickstart ROM: place "Kickstart v1.3 rev 34.5 (1987)(Commodore)(A500-A1000-A2000-CDTV).rom"
# (or equivalent) in ~/Documents/FS-UAE/Kickstarts/ — FS-UAE auto-discovers it.

[config]
amiga_model = A500
chip_memory = 512
slow_memory = 512
fullscreen = 0
video_format = PAL
```

Keep the set of options minimal — anything beyond these (audio buffers, volume, etc.) gets added later only when a real need surfaces. FS-UAE's defaults are sensible.

- [ ] **Step 2: Verify FS-UAE parses without error**

Run: `timeout 3 fs-uae amiga/config/a500.fs-uae 2>&1 | head -40`

Expected: FS-UAE prints its startup banner. Acceptable outcomes:
- FS-UAE complains about a missing Kickstart ROM (proves the config parsed cleanly).
- A window opens briefly and `timeout` kills the process.

Unacceptable: any "unrecognized option", "syntax error", or "could not parse" message referring to a line in the config. Fix the config before moving on.

If FS-UAE is not installed yet, run `sudo apt install fs-uae` first.

- [ ] **Step 3: Commit**

```bash
git add amiga/config/a500.fs-uae
git commit -m "feat(amiga): add A500 FS-UAE config template"
```

---

## Task 5: Write the A1200 FS-UAE config template

**Files:**
- Create: `amiga/config/a1200.fs-uae`

- [ ] **Step 1: Create the config file**

Write `amiga/config/a1200.fs-uae` with this content:

```ini
# FS-UAE configuration: Amiga 1200 (Kickstart 3.1 / AGA).
# Used for early/mid-90s Amiga games requiring AGA chipset. Pair with an .adf via:
#   fs-uae a1200.fs-uae --floppy_drive_0=/path/to/game.adf
#
# Kickstart ROM: place "Kickstart v3.1 rev 40.68 (1993)(Commodore)(A1200).rom"
# (or equivalent) in ~/Documents/FS-UAE/Kickstarts/ — FS-UAE auto-discovers it.

[config]
amiga_model = A1200
chip_memory = 2048
fast_memory = 8192
fullscreen = 0
video_format = PAL
```

- [ ] **Step 2: Verify FS-UAE parses without error**

Run: `timeout 3 fs-uae amiga/config/a1200.fs-uae 2>&1 | head -40`

Expected: same as Task 4 — startup banner plus either a Kickstart-missing error or a window that gets timeout-killed. No syntax errors or unrecognized-option warnings referring to lines in the config.

- [ ] **Step 3: Commit**

```bash
git add amiga/config/a1200.fs-uae
git commit -m "feat(amiga): add A1200 FS-UAE config template"
```

---

## Task 6: Write the test harness for amiga-run.sh

**Files:**
- Create: `amiga/scripts/test-amiga-run.sh`

These tests assert the script's contract before the script exists. Each test runs the (not-yet-existing) script and checks exit code + output. The harness is intentionally simple bash — no bats dependency — because the script retires in Phase 2.

- [ ] **Step 1: Create the test harness**

Write `amiga/scripts/test-amiga-run.sh`:

```bash
#!/bin/bash
# Minimal contract tests for amiga-run.sh.
# Usage: ./test-amiga-run.sh
# Each test prints "PASS"/"FAIL"; the script exits non-zero if any test fails.

set -uo pipefail

SCRIPT="$(cd "$(dirname "$0")" && pwd)/amiga-run.sh"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

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
    out=$("$SCRIPT" "$TMPDIR/nope.adf" 2>&1); local ec=$?
    [[ $ec -eq 2 && "$out" == *"not found"* ]]
}

# Test 4: unsupported extension → exit 2
test_bad_extension() {
    local f="$TMPDIR/foo.zip"; touch "$f"
    local out
    out=$("$SCRIPT" "$f" 2>&1); local ec=$?
    [[ $ec -eq 2 && "$out" == *unsupported* ]]
}

# Test 5: --dry-run with .adf → exit 0, prints fs-uae cmd with --floppy_drive_0
test_dry_run_adf() {
    local f="$TMPDIR/game.adf"; touch "$f"
    local out
    out=$("$SCRIPT" --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *fs-uae* && "$out" == *floppy_drive_0* && "$out" == *a500.fs-uae* ]]
}

# Test 6: --dry-run with .rp9 → exit 0, prints fs-uae cmd without --floppy_drive_0
test_dry_run_rp9() {
    local f="$TMPDIR/game.rp9"; touch "$f"
    local out
    out=$("$SCRIPT" --dry-run "$f" 2>&1); local ec=$?
    [[ $ec -eq 0 && "$out" == *fs-uae* && "$out" != *floppy_drive_0* ]]
}

# Test 7: --model override with .adf → uses specified model config
test_dry_run_model_override() {
    local f="$TMPDIR/game.adf"; touch "$f"
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
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x amiga/scripts/test-amiga-run.sh`

- [ ] **Step 3: Run the tests to confirm they fail (script doesn't exist yet)**

Run: `./amiga/scripts/test-amiga-run.sh`
Expected: every test FAILs because `amiga-run.sh` does not exist. The script still exits non-zero. This is the red phase.

- [ ] **Step 4: Commit**

```bash
git add amiga/scripts/test-amiga-run.sh
git commit -m "test(amiga): add contract tests for amiga-run.sh launcher"
```

---

## Task 7: Implement amiga-run.sh to pass all tests

**Files:**
- Create: `amiga/scripts/amiga-run.sh`

- [ ] **Step 1: Write the script**

Write `amiga/scripts/amiga-run.sh`:

```bash
#!/bin/bash
# Launch an Amiga game in FS-UAE.
#
# Usage:
#   amiga-run.sh [--model <a500|a1200>] [--dry-run] <file>
#   amiga-run.sh --help
#
# Behavior:
#   *.adf  -> pairs the file with a model config (default: a500) via
#             fs-uae <config> --floppy_drive_0=<file>
#   *.rp9  -> hands the bundle directly to fs-uae; model flag ignored
#             (the .rp9 bundle includes its own config)

set -uo pipefail

usage() {
    cat <<EOF
Usage: $(basename "$0") [--model <a500|a1200>] [--dry-run] <file>
       $(basename "$0") --help

Launch an Amiga game in FS-UAE. <file> must be an .adf or .rp9 file.

Options:
  --model <name>   Model config to use with .adf files (default: a500).
                   Resolves to amiga/config/<name>.fs-uae.
  --dry-run        Print the fs-uae command that would run; do not execute.
  -h, --help       Show this help.
EOF
}

MODEL="a500"
DRY_RUN=false
FILE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --model)
            if [[ $# -lt 2 ]]; then
                echo "Error: --model requires a value" >&2
                exit 2
            fi
            MODEL="$2"; shift 2 ;;
        --dry-run)
            DRY_RUN=true; shift ;;
        -h|--help)
            usage; exit 0 ;;
        --) shift; break ;;
        -*)
            echo "Error: unknown option: $1" >&2; usage >&2; exit 2 ;;
        *)
            if [[ -n "$FILE" ]]; then
                echo "Error: only one file argument is supported" >&2; exit 2
            fi
            FILE="$1"; shift ;;
    esac
done

if [[ -z "$FILE" ]]; then
    usage >&2
    exit 2
fi

if [[ ! -f "$FILE" ]]; then
    echo "Error: file not found: $FILE" >&2
    exit 2
fi

FILE_ABS="$(readlink -f "$FILE")"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CONFIG_DIR="$SCRIPT_DIR/../config"

case "$FILE_ABS" in
    *.rp9)
        cmd=(fs-uae "$FILE_ABS")
        ;;
    *.adf)
        cfg="$CONFIG_DIR/$MODEL.fs-uae"
        if [[ ! -f "$cfg" ]]; then
            echo "Error: model config not found: $cfg" >&2
            echo "       Available models: $(ls "$CONFIG_DIR"/*.fs-uae 2>/dev/null | xargs -n1 basename | sed 's/\.fs-uae$//' | tr '\n' ' ')" >&2
            exit 2
        fi
        cmd=(fs-uae "$cfg" "--floppy_drive_0=$FILE_ABS")
        ;;
    *)
        echo "Error: unsupported file extension. Use .adf or .rp9." >&2
        exit 2
        ;;
esac

if $DRY_RUN; then
    printf '%q ' "${cmd[@]}"
    echo
    exit 0
fi

exec "${cmd[@]}"
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x amiga/scripts/amiga-run.sh`

- [ ] **Step 3: Run the tests; expect all pass**

Run: `./amiga/scripts/test-amiga-run.sh`
Expected: `Summary: 7 passed, 0 failed`. Exit code 0.

- [ ] **Step 4: Run shellcheck**

If shellcheck is not installed: `sudo apt install shellcheck`.

Run: `shellcheck amiga/scripts/amiga-run.sh amiga/scripts/test-amiga-run.sh`
Expected: no warnings. Fix any reported issues before committing.

- [ ] **Step 5: Commit**

```bash
git add amiga/scripts/amiga-run.sh
git commit -m "feat(amiga): add generic FS-UAE launcher script (amiga-run.sh)"
```

---

## Task 8: Write the Amiga collection guide

**Files:**
- Create: `amiga/amiga.md`

- [ ] **Step 1: Write the guide**

Write `amiga/amiga.md`:

```markdown
# Amiga Collection

This guide covers running Amiga software in [FS-UAE](https://fs-uae.net/) using a single generic launcher script. Unlike the DOS collections, there is no per-game script — drop your Amiga files into `amiga/games/` and launch any of them by path.

## Prerequisites

Complete the steps in [docs/prerequisites.md](../docs/prerequisites.md), specifically the FS-UAE section.

## Kickstart ROMs

FS-UAE needs the Amiga firmware (Kickstart ROMs) to boot software. These are copyrighted and not distributed with FS-UAE — you must supply your own legally obtained copies (for example, via the [Amiga Forever](https://www.amigaforever.com/) package from Cloanto).

FS-UAE searches `~/Documents/FS-UAE/Kickstarts/` by default. Create that directory and place your ROMs there:

```bash
mkdir -p ~/Documents/FS-UAE/Kickstarts/
cp /path/to/your/kickstart-*.rom ~/Documents/FS-UAE/Kickstarts/
```

For the A500 config template, FS-UAE expects a Kickstart 1.3 ROM. For the A1200 template, a Kickstart 3.1 ROM. FS-UAE will print a clear error at launch if the required ROM is missing.

## Supported file formats

| Format | What it is | How the launcher handles it |
|---|---|---|
| `.adf` | Raw Amiga floppy disk image (single disk). | Paired with a model config (default: A500); inserted into floppy drive 0. |
| `.rp9` | RetroPlatform bundle — self-contained zip with config, ROMs metadata, and disk images. | Passed directly to FS-UAE, which reads model + config from the bundle. |

WHDLoad (`.hdf`, `.lha`) and multi-disk games requiring disk swapping are not covered by Phase 1 of this collection.

## Adding games

Drop your `.adf` or `.rp9` files into `amiga/games/`:

```bash
cp /path/to/lemmings.adf amiga/games/
```

Files in `amiga/games/` are gitignored.

## Running a game

From `amiga/scripts/`:

```bash
cd amiga/scripts
./amiga-run.sh ../games/lemmings.adf
```

For an A1200-targeted game:

```bash
./amiga-run.sh --model a1200 ../games/pinball-illusions.adf
```

For an `.rp9` bundle (the model flag is ignored — the bundle drives the config):

```bash
./amiga-run.sh ../games/turrican.rp9
```

To preview the FS-UAE command without launching:

```bash
./amiga-run.sh --dry-run ../games/lemmings.adf
```

## Model config templates

| File | Amiga model | Kickstart | Use for |
|---|---|---|---|
| `amiga/config/a500.fs-uae` | A500 | 1.3 (OCS) | Most late-80s Amiga games. |
| `amiga/config/a1200.fs-uae` | A1200 | 3.1 (AGA) | Early/mid-90s AGA-required games. |

To tweak a template (e.g. enable fullscreen, change audio settings), edit the file directly. Changes apply on the next launch.

## Troubleshooting

- **"Kickstart ROM not found"** — FS-UAE could not locate a Kickstart ROM in `~/Documents/FS-UAE/Kickstarts/`. Verify the file is there and named consistently with what FS-UAE expects (filenames don't have to match exactly; FS-UAE inspects the ROM contents).
- **Game launches but mouse/keyboard input doesn't reach the Amiga** — click inside the FS-UAE window to grab input. Press `F12` to open the in-emulator menu.
- **`./amiga-run.sh: command not found`** — the script needs to be executable. Run `chmod +x amiga/scripts/amiga-run.sh`.
```

- [ ] **Step 2: Sanity-check the guide renders**

Run: `head -60 amiga/amiga.md`
Expected: well-formed markdown, no obviously broken fences or tables.

- [ ] **Step 3: Commit**

```bash
git add amiga/amiga.md
git commit -m "docs(amiga): add user-facing guide for Amiga collection"
```

---

## Task 9: Update the top-level README to list the Amiga collection

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add the Amiga collection link**

In `README.md`, update the Game Guides list. Currently:

```markdown
- [King's Quest](dos/kings-quest/kings-quest.md)
- [Quest for Glory](dos/quest-for-glory/quest-for-glory.md)
```

Change to (keep alphabetical):

```markdown
- [Amiga](amiga/amiga.md)
- [King's Quest](dos/kings-quest/kings-quest.md)
- [Quest for Glory](dos/quest-for-glory/quest-for-glory.md)
```

- [ ] **Step 2: Verify**

Run: `cat README.md`
Expected: the Amiga entry appears in the list, ahead of King's Quest.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs(readme): list Amiga collection in game guides"
```

---

## Task 10: End-to-end verification with a real .adf

This task confirms the whole Phase 1 deliverable works on a clean system. It requires the user (or executor) to have a legally obtained Kickstart 1.3 ROM and a known-good `.adf` available.

- [ ] **Step 1: Install FS-UAE (if not already installed)**

Run: `sudo apt install fs-uae`
Expected: install succeeds; `which fs-uae` returns a path.

- [ ] **Step 2: Place a Kickstart ROM**

Manual: copy your Kickstart 1.3 ROM into `~/Documents/FS-UAE/Kickstarts/`. (FS-UAE creates this directory on first run; create it manually if it doesn't exist.)

- [ ] **Step 3: Place a test .adf in the games drop directory**

Manual: `cp /path/to/your/test-game.adf amiga/games/`

- [ ] **Step 4: Launch via the script**

Run:
```bash
cd amiga/scripts
./amiga-run.sh ../games/test-game.adf
```

Expected: FS-UAE window opens, the floppy boots, the game runs.

- [ ] **Step 5: Repeat with a .rp9 bundle (optional)**

If you have an `.rp9` available, place it in `amiga/games/` and run:
```bash
./amiga-run.sh ../games/test-bundle.rp9
```

Expected: FS-UAE opens with the bundle's embedded config, software runs.

- [ ] **Step 6: Confirm the test harness still passes**

Run: `./amiga/scripts/test-amiga-run.sh`
Expected: `Summary: 7 passed, 0 failed`.

No commit on this task — it's verification only. If anything fails, the failing task above is the place to fix it.

---

## Self-review checklist (for the implementer)

Before marking Phase 1 complete:

- [ ] All 10 tasks completed and committed.
- [ ] `./amiga/scripts/test-amiga-run.sh` exits 0.
- [ ] `shellcheck amiga/scripts/*.sh` reports no warnings.
- [ ] At least one real `.adf` launches end-to-end (Task 10).
- [ ] `README.md`, `docs/prerequisites.md`, and `amiga/amiga.md` cross-reference each other correctly (Amiga entry in README links to `amiga/amiga.md`; `amiga.md` links back to `docs/prerequisites.md`).
- [ ] `git status` is clean.

When all of these are true, Phase 1 is done. Phase 2 (manifest schema + Rust CLI engine) gets its own implementation plan when we're ready to start.
