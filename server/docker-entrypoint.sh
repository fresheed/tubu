#!/usr/bin/env bash
set -euo pipefail

./preprocess.sh video.mp4 dash
exec python3 server.py "${PORT:-8000}"
