#!/bin/zsh
# Captures the README media from a real kitty window: demo.png (a still)
# and demo.mov (an edit session). Terminal recorders such as vhs cannot
# show kitty graphics, so this uses macOS screencapture.
#
# Usage: media/record.sh [outdir]
# Needs: macOS, kitty, swift, and `cargo build --release` in helper/.
set -e
M=${0:a:h}
OUT=${1:-$M}
W=$(mktemp -d)

launch() {
  cp $M/demo.md $M/banner.png $W/
  KMD_DEMO_SCRIPT=$1 kitty --single-instance=no --title kmd-demo \
    -o remember_window_size=no -o initial_window_width=1280 \
    -o initial_window_height=800 -o font_size=13 --directory $W \
    nvim --clean -n -u $M/init.lua demo.md >/dev/null 2>&1 &
  echo $!
}

window_id() {
  swift $M/winid.swift $1 2>/dev/null | awk '$2 == 1280 { print $1; exit }'
}

pid=$(launch 0)
sleep 7
screencapture -x -o -l$(window_id $pid) $OUT/demo.png
kill $pid

pid=$(launch 1)
sleep 3
screencapture -x -o -v -V 22 -l$(window_id $pid) $OUT/demo.mov
kill $pid

rm -rf $W
