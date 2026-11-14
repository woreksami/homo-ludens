import sys, json
sys.path.insert(0, "harness")
import calibrate
from uci_client import UciEngine
from pathlib import Path
import chess
pos = calibrate.sample(Path("reports/pro/humans.pgn"), 250, 200, 7)[:120]
e = UciEngine(["bin/hl_v4"], {"ShowThoughts": "false"})
sc = calibrate.Scorer("../dl/sf/src/stockfish", 30000, Path("reports/pro/calib_cache.json"))
for i, (_, start, moves, human, _) in enumerate(pos):
    m = e.play(start, moves, go="movetime 200").move
    b = chess.Board()
    for x in moves: b.push_uci(x)
    l = sc.loss(b.fen(), m)
    if l >= 150:
        best = sc.eng.analyse(b, chess.engine.Limit(nodes=30000))
        print(l, b.fen(), "hl", m, "sf", best["pv"][0].uci(), best["score"].pov(b.turn), "human", human, sc.loss(b.fen(), human))
