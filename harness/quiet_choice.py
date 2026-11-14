"""Quick check on the 'several good moves' giveaway, no stockfish run needed.
Positions (ply >= 8) where the cached multipv has 2+ good moves (within 5% win
chance); how often do humans vs HL pick the top move / a top-3 move.
"""
import argparse, json, math, random, sys
from pathlib import Path
import chess, chess.pgn
sys.path.insert(0, str(Path(__file__).resolve().parent))
from uci_client import UciEngine

def wp(cp):
    cp = max(-1000, min(1000, cp))
    return 50 + 50 * (2 / (1 + math.exp(-0.00368208 * cp)) - 1)

ap = argparse.ArgumentParser()
ap.add_argument("--pgn", type=Path, required=True)
ap.add_argument("--analysis", type=Path, required=True)
ap.add_argument("--hl", required=True)
ap.add_argument("--n", type=int, default=400)
ap.add_argument("--movetime", type=int, default=200)
ap.add_argument("--opt", action="append", default=[], help="extra UCI option name=value")
a = ap.parse_args()
an = {}
for line in open(a.analysis):
    r = json.loads(line); an[r["fen"]] = r
pos = []
with open(a.pgn) as f:
    while (g := chess.pgn.read_game(f)) is not None:
        b = g.board(); moves = []
        for i, m in enumerate(g.mainline_moves()):
            r = an.get(b.fen())
            if i >= 8 and r and len(r["lines"]) >= 2:
                best = wp(r["lines"][0][1])
                good = [u for u, cp in r["lines"] if best - wp(cp) < 5]
                second = best - wp(r["lines"][1][1])
                if len(good) >= 2 and second < 10:
                    elo = int(g.headers["WhiteElo" if b.turn else "BlackElo"])
                    pos.append((list(moves), m.uci(), elo, [u for u, _ in r["lines"]]))
            moves.append(m.uci()); b.push(m)
random.Random(1).shuffle(pos); pos = pos[: a.n]
opts = {"UCI_LimitStrength": "true", "ShowThoughts": "false"}
for o in a.opt:
    k, v = o.split("="); opts[k] = v
e = UciEngine([a.hl], opts)
h1 = h3 = b1 = b3 = 0
for i, (moves, human, elo, lines) in enumerate(pos):
    e.set_option("UCI_Elo", elo); e.set_option("Seed", i + 1)
    m = e.play(chess.STARTING_FEN, moves, go=f"movetime {a.movetime}").move
    h1 += human == lines[0]; h3 += human in lines
    b1 += m == lines[0]; b3 += m in lines
e.close()
n = len(pos)
print(f"{n} positions  top1 human {h1/n:.3f} HL {b1/n:.3f}   top3 human {h3/n:.3f} HL {b3/n:.3f}")
