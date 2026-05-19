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
            echo "       Available models: $(find "$CONFIG_DIR" -maxdepth 1 -name '*.fs-uae' -print0 | xargs -0 -n1 basename | sed 's/\.fs-uae$//' | tr '\n' ' ')" >&2
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
