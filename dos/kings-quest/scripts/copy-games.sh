#! /bin/bash
set -e

# Default values
force=false

usage() {
>&2 cat << EOF
Usage: $0
   [ -f | --force ]
   [ -h | --help ]

Copies King's Quest game files from ../games/<game>/ to ~/games/<game>/
for each of the six games (kq1sci, kq2, kq3, kq4, kq5, kq6).

Game files in ../games/ must be populated first by copying them from a
Steam install (see kings-quest.md).

Options:
  -f, --force   Overwrite an existing ~/games/<game>/ destination
                (use after re-copying fresh files from Steam).
EOF
exit 1
}

args=$(getopt -a -o fh --long force,help -- "$@")
if [[ $? -gt 0 ]]; then
  usage
fi

eval set -- ${args}
while :
do
  case $1 in
    -f | --force)   force=true   ; shift   ;;
    -h | --help)    usage        ; shift   ;;
    --) shift; break ;;
    *) >&2 echo Unsupported option: $1
       usage ;;
  esac
done

# create ~/games directory if it doesn't exist
mkdir -p ~/games

echo "Copying King's Quest games to ~/games..."

for gamedir in kq1sci kq2 kq3 kq4 kq5 kq6; do
    src="../games/$gamedir"
    dest=~/games/$gamedir

    if [ ! -d "$src" ] || [ -z "$(ls -A "$src" 2>/dev/null)" ]; then
        echo "  $src is missing or empty; skipping (copy this game's files from Steam first)"
        continue
    fi

    if [ -d "$dest" ] && [ "$force" != true ]; then
        echo "  $dest already exists; skipping (preserves any in-game config; pass -f to overwrite)"
        continue
    fi

    if [ -d "$dest" ] && [ "$force" = true ]; then
        rm -rf "$dest"
    fi

    cp -r "$src" ~/games/
    echo "  Copied $gamedir to $dest"
done
