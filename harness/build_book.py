"""Opening book by rating band from real games.

For each position in the first --max-ply plies, count what the mover played,
split by their band. HL samples from these counts so its openings look like
what people at its rating actually play.

Output lines: band<TAB>fen4<TAB>uci:count uci:count ...
Games in --exclude (matched on site, players, date, round) are left out so
test games don't leak into the book.
"""

from __future__ import annotations

import argparse
from collections import defaultdict
from pathlib import Path

import chess
import chess.pgn


def fen4(b: chess.Board) -> str:
    return " ".join(b.fen().split()[:3])


def game_key(h) -> tuple:
    return (h.get("Site", ""), h.get("White", ""), h.get("Black", ""), h.get("Date", ""), h.get("Round", ""))


def sites(pgn: Path) -> set[tuple]:
    out = set()
    with open(pgn, encoding="utf-8", errors="replace") as f:
        while (h := chess.pgn.read_headers(f)) is not None:
            out.add(game_key(h))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pgn", nargs="+", type=Path, required=True)
    ap.add_argument("--exclude", nargs="*", type=Path, default=[])
    ap.add_argument("--max-ply", type=int, default=20)
    ap.add_argument("--band", type=int, default=200)
    ap.add_argument("--min-count", type=int, default=2)
    ap.add_argument("--out", type=Path, default=Path("data/book.tsv"))
    args = ap.parse_args()

    skip = set().union(*(sites(p) for p in args.exclude)) if args.exclude else set()
    counts: dict[tuple[int, str], dict[str, int]] = defaultdict(lambda: defaultdict(int))
    games = 0
    for pgn in args.pgn:
        with open(pgn, encoding="utf-8", errors="replace") as f:
            while (g := chess.pgn.read_game(f)) is not None:
                if game_key(g.headers) in skip:
                    continue
                try:
                    elo = {chess.WHITE: int(g.headers["WhiteElo"]), chess.BLACK: int(g.headers["BlackElo"])}
                except (KeyError, ValueError):
                    continue
                games += 1
                b = g.board()
                for i, m in enumerate(g.mainline_moves()):
                    if i >= args.max_ply:
                        break
                    band = elo[b.turn] // args.band * args.band
                    counts[(band, fen4(b))][m.uci()] += 1
                    b.push(m)
    n = 0
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w") as f:
        for (band, fen), mv in sorted(counts.items()):
            if sum(mv.values()) < args.min_count:
                continue
            f.write(f"{band}\t{fen}\t{' '.join(f'{u}:{c}' for u, c in sorted(mv.items(), key=lambda x: -x[1]))}\n")
            n += 1
    print(f"{games} games -> {n} book entries in {args.out}")


if __name__ == "__main__":
    main()
