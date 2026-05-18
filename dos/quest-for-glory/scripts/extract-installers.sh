#! /bin/bash
set -e

# Default values
copy_games_dir=false
create_desktop_icons=false

usage() {
>&2 cat << EOF
Usage: $0
   [ -c | --copy-games-dir ]
   [ -i | --create-desktop-icons ]
   [ -h | --help ]
   <infile> [infiles]
EOF
exit 1
}

args=$(getopt -a -o cih --long copy-games-dir,create-desktop-icons,help -- "$@")
if [[ $? -gt 0 ]]; then
  usage
fi

eval set -- ${args}
while :
do
  case $1 in
    -c | --copy-games-dir)         copy_games_dir=true           ; shift   ;;
    -i | --create-desktop-icons)    create_desktop_icons=true    ; shift   ;;
    -h | --help)                    usage                        ; shift   ;;
    # -- means the end of the arguments; drop this, and break out of the while loop
    --) shift; break ;;
    *) >&2 echo Unsupported option: $1
       usage ;;
  esac
done

# check if game installers are available
if [ ! -n "$(find ../installers -maxdepth 1 -name "*.exe")" ]; then
   echo "Game installers are missing. Please add installers to the /installers directory."
   
   exit 1
fi

# create /games directory if it doesn't exist
if [ ! -d "../games" ]; then
    echo "Creating /games directory..."
    
    mkdir ../games
fi

# extract QFG1
echo "Extracting QFG1 games files..."

if [ ! -d "../games/qfg1-vga" ]; then
    mkdir ../games/qfg1-vga
fi

innoextract --exclude-temp --silent --output-dir ../games/qfg1-vga ../installers/qfg1.exe

if [ -d "../games/qfg1-ega" ]; then
    rm -rf ../games/qfg1-ega
fi

mv ../games/qfg1-vga/EGA ../games/qfg1-ega

# extract QFG2
echo "Extracting QFG2 games files ..."

if [ ! -d "../games/qfg2" ]; then
    mkdir ../games/qfg2
fi

innoextract --exclude-temp --silent --output-dir ../games/qfg2 ../installers/qfg2.exe

# extract QFG3
echo "Extracting QFG3 game files..."

if [ ! -d "../games/qfg3" ]; then
    mkdir ../games/qfg3
fi

innoextract --exclude-temp --silent --output-dir ../games/qfg3 ../installers/qfg3.exe

# extract QFG4
echo "Extracting QFG4 games files..."
if [ ! -d "../games/qfg4" ]; then
    mkdir ../games/qfg4
fi

innoextract --exclude-temp --silent --output-dir ../games/qfg4 ../installers/qfg4.exe

# copy games to ~/games if -c was passed
if [ "$copy_games_dir" = true ]; then
    echo "Copying games to ~/games..."
    mkdir -p ~/games
    for gamedir in qfg1-vga qfg1-ega qfg2 qfg3 qfg4; do
        if [ -d ~/games/$gamedir ]; then
            echo "  ~/games/$gamedir already exists; skipping (preserves any in-game config)"
        else
            cp -r ../games/$gamedir ~/games/
            echo "  Copied $gamedir to ~/games/$gamedir"
        fi
    done
fi

# TODO: create desktop icons