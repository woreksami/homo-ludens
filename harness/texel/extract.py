"""Sample quiet positions with game results from PGNs, as feature rows for tuning."""

from __future__ import annotations

import argparse
import random
import sys
from multiprocessing import Pool
from pathlib import Path

import chess
import chess.pgn
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from features import N_TERMS, features  # noqa: E402

RES = {"1-0": 1.0, "0-1": 0.0, "1/2-1/2": 0.5}


def games(pgn, skip):
    with open(pgn, encoding="utf-8", errors="replace") as f:
        while (g := chess.pgn.read_game(f)) is not None:
            h = g.headers
            if h.get("Result") not in RES:
                continue
            key = (h.get("Site", ""), h.get("White", ""), h.get("Black", ""), h.get("Date", ""), h.get("Round", ""))
            if key in skip:
                continue
            yield RES[h["Result"]], [m.uci() for m in g.mainline_moves()]


def work(item):
    res, moves, per_game, seed = item
    rng = random.Random(seed)
    b = chess.Board()
    cands = []
    for i, u in enumerate(moves):
        m = chess.Move.from_uci(u)
        quiet_move = not b.is_capture(m) and m.promotion is None
        b.push(m)
        if i >= 16 and quiet_move and not b.is_check() and not b.is_game_over():
            cands.append(b.fen())
    rng.shuffle(cands)
    out = []
    for fen in cands[:per_game]:
        bb = chess.Board(fen)
        if any(bb.is_capture(m) for m in bb.legal_moves if bb.piece_at(m.to_square) and bb.piece_at(m.to_square).piece_type >= chess.KNIGHT):
            continue  # a piece hangs or can be traded: not quiet enough
        x, ph = features(bb)
        out.append((x, ph, res))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pgn", nargs="+", required=True)
    ap.add_argument("--exclude", nargs="*", default=[])
    ap.add_argument("--per-game", type=int, default=10)
    ap.add_argument("--max-games", type=int, default=40000)
    ap.add_argument("--workers", type=int, default=2)
    ap.add_argument("--out", default="reports/texel/data.npz")
    args = ap.parse_args()
    skip = set()
    for p in args.exclude:
        with open(p, encoding="utf-8", errors="replace") as f:
            while (h := chess.pgn.read_headers(f)) is not None:
                skip.add((h.get("Site", ""), h.get("White", ""), h.get("Black", ""), h.get("Date", ""), h.get("Round", "")))
    items = []
    for p in args.pgn:
        for res, moves in games(p, skip):
            items.append((res, moves, args.per_game, len(items)))
            if len(items) >= args.max_games:
                break
    print(f"{len(items)} games", file=sys.stderr)
    rows = []
    with Pool(args.workers) as pool:
        for i, out in enumerate(pool.imap_unordered(work, items, chunksize=50)):
            rows.extend(out)
            if i % 5000 == 0:
                print(f"  {i} games, {len(rows)} positions", file=sys.stderr)
    X = np.zeros((len(rows), N_TERMS), dtype=np.float32)
    ph = np.zeros(len(rows), dtype=np.float32)
    y = np.zeros(len(rows), dtype=np.float32)
    for i, (x, p, r) in enumerate(rows):
        for t, v in x.items():
            X[i, t] = v
        ph[i] = p
        y[i] = r
    Path(args.out).parent.mkdir(parents=True, exist_ok=True)
    np.savez_compressed(args.out, X=X, ph=ph, y=y)
    print(f"{len(rows)} positions -> {args.out}", file=sys.stderr)


if __name__ == "__main__":
    main()
