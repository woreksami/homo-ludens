"""Sample (position, last-move square, played move, rating) from human games for the naturalness fit."""
import argparse, random, sys
from pathlib import Path
import chess, chess.pgn
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build_book import game_key

ap = argparse.ArgumentParser()
ap.add_argument("--pgn", type=Path, required=True)
ap.add_argument("--exclude", type=Path, nargs="*", default=[])
ap.add_argument("--per-game", type=int, default=6)
ap.add_argument("--out", type=Path, required=True)
a = ap.parse_args()
skip = set()
for p in a.exclude:
    with open(p, errors="replace") as f:
        while (g := chess.pgn.read_game(f)) is not None:
            skip.add(game_key(g.headers))
rng = random.Random(5)
rows = []
with open(a.pgn, errors="replace") as f:
    while (g := chess.pgn.read_game(f)) is not None:
        h = g.headers
        if game_key(h) in skip or "?" in h.get("WhiteElo", "?") + h.get("BlackElo", "?"):
            continue
        b = g.board(); last = "-"; cand = []
        for i, m in enumerate(g.mainline_moves()):
            if 4 <= i < 160:
                elo = h["WhiteElo"] if b.turn else h["BlackElo"]
                cand.append(f"{b.fen()}\t{last}\t{m.uci()}\t{elo}")
            last = chess.square_name(m.to_square); b.push(m)
        rows += rng.sample(cand, min(a.per_game, len(cand)))
rng.shuffle(rows)
a.out.write_text("\n".join(rows) + "\n")
print(len(rows), "positions", file=sys.stderr)
