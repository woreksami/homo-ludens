#!/usr/bin/env bash
# Download one month of the Lichess open database and keep a manageable slice.
#
#   ./harness/fetch_lichess.sh 2025-01 200000
#
# Arguments: month (YYYY-MM, default 2025-01), number of games to keep
# (default 200000). Needs curl and zstd. A full month is 30+ GB compressed;
# this streams it and stops after the requested number of games.
# Data licence: CC0, see https://database.lichess.org
set -euo pipefail
MONTH="${1:-2025-01}"
GAMES="${2:-200000}"
mkdir -p data
OUT="data/lichess_${MONTH}_${GAMES}.pgn"
URL="https://database.lichess.org/standard/lichess_db_standard_rated_${MONTH}.pgn.zst"
echo "Streaming ${URL} -> ${OUT} (first ${GAMES} games)"
curl -sSL "$URL" | zstd -dc | awk -v n="$GAMES" '
  /^\[Event / { games++; if (games > n) exit }
  { print }
' > "$OUT" || true
echo "Wrote $(grep -c '^\[Event ' "$OUT") games to $OUT"
