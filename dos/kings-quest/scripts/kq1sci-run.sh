#! /bin/bash

# Start FluidSynth
fluidsynth -i /usr/share/sounds/sf2/FluidR3_GM.sf2 &

# Wait for FluidSynth to start
while ! pgrep -x "fluidsynth" > /dev/null; do
    sleep 1
done

# Run King's Quest 1 (SCI remake) in DosBox Staging
flatpak run io.github.dosbox-staging -conf ../config/kq1sci.conf
