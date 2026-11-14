import sys, json, statistics
sys.path.insert(0, "harness")
import calibrate
from uci_client import UciEngine
from pathlib import Path
import chess
hl, mt, lim = sys.argv[1], int(sys.argv[2]), sys.argv[3]
pos = calibrate.sample(Path("reports/pro/humans.pgn"), 250, 200, 7)[:int(sys.argv[4])]
opts = {"ShowThoughts": "false", "UCI_LimitStrength": "false"}
if lim != "full": opts.update({"UCI_LimitStrength": "true", "UCI_Elo": lim})
e = UciEngine([hl], opts)
sc = calibrate.Scorer("../dl/sf/src/stockfish", 30000, Path("reports/pro/calib_cache.json"))
L = []
for i, (_, start, moves, human, _) in enumerate(pos):
    e.set_option("Seed", 1 + i)
    m = e.play(start, moves, go=f"movetime {mt}").move
    b = chess.Board()
    for x in moves: b.push_uci(x)
    L.append(sc.loss(b.fen(), m))
sc.save(); sc.eng.quit(); e.close()
print(hl, mt, lim, "acpl", round(statistics.fmean(L),1), "blunder", sum(x>=200 for x in L)/len(L))
