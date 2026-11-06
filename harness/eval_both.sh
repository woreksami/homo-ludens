#!/bin/bash
# usage: eval_both.sh <version> <prev-version-for-cache>   (env vars pass through to HL)
V=$1; PREV=$2; cd /home/claude/homo_ludens
for set in k old; do
  if [ $set = k ]; then D=reports/k_$V; P=data/kaggle/lichess_2017.pgn; B=1000,1200,1400,1600,1800,2000,2200; N=70; C=reports/k_$PREV/analysis_50000.jsonl
  else D=reports/anticheat_$V; P=reports/anticheat_big2/humans.pgn; B=1000,1200,1400,1600,1800,2000; N=60; C=reports/anticheat_$PREV/analysis_50000.jsonl; fi
  mkdir -p $D; cp $C $D/
  python3 harness/anticheat/make_games.py --pgn $P --bands $B --hl bin/hl_$V --bots homo_ludens --per-band $N --allow-resign --out-dir $D > $D/run.log 2>&1 && \
  python3 harness/anticheat/analyse.py --stockfish ../dl/sf/src/stockfish --nodes 50000 --pgns $D/humans.pgn $D/homo_ludens.pgn >> $D/run.log 2>&1 && \
  python3 harness/anticheat/grade.py --dir $D --bots homo_ludens --nodes 50000 >> $D/run.log 2>&1
  python3 harness/conversion.py $D humans homo_ludens >> $D/run.log 2>&1
done
echo finished $V
