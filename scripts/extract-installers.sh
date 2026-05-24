#!/bin/bash
#
# Extract the four Quest for Glory GOG installers into ~/games/.
# Run with: scripts/extract-installers.sh (from anywhere — the script
# resolves paths relative to its own location).
#
# Expects the four .exe files at dos/quest-for-glory/installers/:
#   qfg1.exe, qfg2.exe, qfg3.exe, qfg4.exe
#
# After extraction, register each install with the launcher via either
# `reliquaint migrate-installs` (one shot) or `reliquaint install <id>
# ~/games/<id>` (per game).

set -e

usage() {
  >&2 cat << EOF
Usage: $0 [-h | --help]

Extracts dos/quest-for-glory/installers/{qfg1,qfg2,qfg3,qfg4}.exe
into ~/games/{qfg1-ega,qfg1-vga,qfg2,qfg3,qfg4}/ using innoextract.
EOF
  exit 1
}

case "${1:-}" in
  -h|--help) usage ;;
  "") ;;
  *) >&2 echo "Unsupported option: $1"; usage ;;
esac

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INSTALLERS_DIR="${SCRIPT_DIR}/../dos/quest-for-glory/installers"

if [ ! -n "$(find "${INSTALLERS_DIR}" -maxdepth 1 -name "*.exe" 2>/dev/null)" ]; then
  echo "No .exe installers found in ${INSTALLERS_DIR}"
  echo "Drop the GOG installer .exe files there (named qfg1.exe through qfg4.exe) and re-run."
  exit 1
fi

mkdir -p ~/games

# QFG1: the GOG installer ships EGA + VGA together. Extract to qfg1-vga,
# then move the EGA subfolder out.
echo "Extracting QFG1 (EGA + VGA)..."
mkdir -p ~/games/qfg1-vga
innoextract --exclude-temp --silent --output-dir ~/games/qfg1-vga "${INSTALLERS_DIR}/qfg1.exe"
if [ -d ~/games/qfg1-ega ]; then
  rm -rf ~/games/qfg1-ega
fi
mv ~/games/qfg1-vga/EGA ~/games/qfg1-ega

for n in 2 3 4; do
  echo "Extracting QFG${n}..."
  mkdir -p "$HOME/games/qfg${n}"
  innoextract --exclude-temp --silent --output-dir "$HOME/games/qfg${n}" "${INSTALLERS_DIR}/qfg${n}.exe"
done

echo
echo "Done. Register the installs with the launcher:"
echo "  reliquaint migrate-installs"
