"""Engine-vs-engine match at full strength, to measure strength changes.

Plays pairs of games from the same opening (each side once), openings taken
from real games (first --open-plies plies), and reports the score with an
Elo estimate and 95% error bar.
"""

from __future__ import annotations

import argparse
import math
import random
import sys

import chess
import chess.engine
import chess.pgn


def openings(pgn: str, n: int, plies: int, seed: int) -> list[list[chess.Move]]:
    rng = random.Random(seed)
    out = []
    with open(pgn, encoding="utf-8", errors="replace") as f:
        while (g := chess.pgn.read_game(f)) is not None and len(out) < n * 20:
            ms = list(g.mainline_moves())[:plies]
            if len(ms) == plies:
                out.append(ms)
    rng.shuffle(out)
    return out[:n]


def play(a, b, opening, limit, max_plies=300):
    board = chess.Board()
    for m in opening:
        board.push(m)
    engines = {chess.WHITE: a, chess.BLACK: b}
    while not board.is_game_over(claim_draw=True) and board.ply() < max_plies:
        r = engines[board.turn].play(board, limit, game=id(opening))
        board.push(r.move)
    res = board.result(claim_draw=True)
    return {"1-0": 1.0, "0-1": 0.0}.get(res, 0.5)


def elo(score: float) -> float:
    score = min(max(score, 1e-3), 1 - 1e-3)
    return -400 * math.log10(1 / score - 1)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("new")
    ap.add_argument("base")
    ap.add_argument("--pgn", default="data/masters/mega2600_part_01.pgn")
    ap.add_argument("--pairs", type=int, default=50)
    ap.add_argument("--open-plies", type=int, default=12)
    ap.add_argument("--movetime", type=int, default=100)
    ap.add_argument("--seed", type=int, default=1)
    args = ap.parse_args()
    opts = {"UCI_LimitStrength": "false", "ShowThoughts": "false"}
    a = chess.engine.SimpleEngine.popen_uci(args.new); a.configure(opts)
    b = chess.engine.SimpleEngine.popen_uci(args.base); b.configure(opts)
    limit = chess.engine.Limit(time=args.movetime / 1000)
    results = []
    for i, op in enumerate(openings(args.pgn, args.pairs, args.open_plies, args.seed)):
        results.append(play(a, b, op, limit))          # new as White
        results.append(1 - play(b, a, op, limit))      # new as Black
        n = len(results); s = sum(results) / n
        sd = math.sqrt(sum((x - s) ** 2 for x in results) / max(1, n - 1)) / math.sqrt(n)
        print(f"{n} games: new scores {s:.3f}  Elo {elo(s):+.0f} ± {elo(min(0.999, s + 1.96 * sd)) - elo(s):.0f}", file=sys.stderr, flush=True)
    a.quit(); b.quit()


if __name__ == "__main__":
    main()
