#!/usr/bin/env bash
# Segments video.mp4 into MPEG-DASH (init + media segments + manifest.mpd)
# under ./dash/. Re-run any time video.mp4 changes.
set -euo pipefail

SRC="${1:-video.mp4}"
OUT_DIR="${2:-dash}"
SEG_DURATION=4

if [[ ! -f "$SRC" ]]; then
    echo "Source file not found: $SRC" >&2
    exit 1
fi

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

ffmpeg -y -i "$SRC" \
    -map 0:v:0 -map 0:a:0 \
    -c:v libx264 -x264-params "keyint=$((SEG_DURATION*25)):scenecut=0" -profile:v main -level 4.0 \
    -c:a aac -b:a 128k \
    -f dash \
    -seg_duration "$SEG_DURATION" \
    -use_template 1 -use_timeline 1 \
    -init_seg_name 'init-$RepresentationID$.m4s' \
    -media_seg_name 'chunk-$RepresentationID$-$Number%05d$.m4s' \
    "$OUT_DIR/manifest.mpd"

echo "DASH output written to $OUT_DIR/manifest.mpd"
