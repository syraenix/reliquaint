#!/bin/bash
# Launch an Amiga game in FS-UAE.
#
# Usage:
#   amiga-run.sh [--model <a500|a1200>] [--windowed] [--dry-run] <file>
#   amiga-run.sh --help
#
# Behavior:
#   *.adf  -> pairs the file with a model config (default: a500) via
#             fs-uae <config> --floppy_drive_0=<file>
#   *.rp9  -> unpacks the bundle (it's a zip) to a temp dir. If it contains a
#             .fs-uae config, that config is used directly. Otherwise the
#             first inner .adf is paired with the chosen model config, same
#             as the .adf path. The bundle's rp9-manifest.xml is ignored.

set -uo pipefail

usage() {
    cat <<EOF
Usage: $(basename "$0") [--model <a500|a1200>] [--windowed] [--dry-run] <file>
       $(basename "$0") --help

Launch an Amiga game in FS-UAE. <file> must be an .adf or .rp9 file.

Options:
  --model <name>   Model config to use with .adf files (default: a500).
                   Resolves to amiga/config/<name>.fs-uae. Also used as the
                   fallback for .rp9 bundles that don't ship a .fs-uae config.
  --windowed       Force windowed mode (passes --fullscreen=0 to fs-uae).
  --dry-run        Print the fs-uae command that would run; do not execute.
  -h, --help       Show this help.
EOF
}

MODEL="a500"
DRY_RUN=false
WINDOWED=false
FILE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --model)
            if [[ $# -lt 2 ]]; then
                echo "Error: --model requires a value" >&2
                exit 2
            fi
            MODEL="$2"; shift 2 ;;
        --windowed)
            WINDOWED=true; shift ;;
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
CONFIG_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/config"

# Build cmd=(fs-uae <model-config> --floppy_drive_0=<adf>). Exits 2 if the
# model config doesn't exist.
build_adf_cmd() {
    local adf="$1"
    local cfg="$CONFIG_DIR/$MODEL.fs-uae"
    if [[ ! -f "$cfg" ]]; then
        echo "Error: model config not found: $cfg" >&2
        echo "       Available models: $(find "$CONFIG_DIR" -maxdepth 1 -name '*.fs-uae' -print0 | xargs -0 -n1 basename | sed 's/\.fs-uae$//' | tr '\n' ' ')" >&2
        exit 2
    fi
    cmd=(fs-uae "$cfg" "--floppy_drive_0=$adf")
}

UNPACK_DIR=""
cleanup() {
    if [[ -n "$UNPACK_DIR" && -d "$UNPACK_DIR" ]]; then
        rm -rf "$UNPACK_DIR"
    fi
}
trap cleanup EXIT

case "$FILE_ABS" in
    *.rp9)
        UNPACK_DIR="$(mktemp -d)"
        if ! unzip -q "$FILE_ABS" -d "$UNPACK_DIR"; then
            echo "Error: failed to unpack .rp9 bundle: $FILE_ABS" >&2
            exit 2
        fi
        inner_cfg="$(find "$UNPACK_DIR" -type f -name '*.fs-uae' -print -quit)"
        if [[ -n "$inner_cfg" ]]; then
            cmd=(fs-uae "$inner_cfg")
        else
            inner_adf="$(find "$UNPACK_DIR" -type f -name '*.adf' -print -quit)"
            if [[ -z "$inner_adf" ]]; then
                echo "Error: .rp9 bundle has no .fs-uae config or .adf file: $FILE_ABS" >&2
                exit 2
            fi
            build_adf_cmd "$inner_adf"
        fi
        ;;
    *.adf)
        build_adf_cmd "$FILE_ABS"
        ;;
    *)
        echo "Error: unsupported file extension. Use .adf or .rp9." >&2
        exit 2
        ;;
esac

if $WINDOWED; then
    cmd+=("--fullscreen=0")
fi

if $DRY_RUN; then
    printf '%q ' "${cmd[@]}"
    echo
    exit 0
fi

"${cmd[@]}"
