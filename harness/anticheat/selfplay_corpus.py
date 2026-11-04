"""Homo Ludens vs itself at fixed ratings, for ratings with no human games to compare.

Writes a PGN with the same headers and clocks as make_games.py replays, so
analyse.py and grade.py's per-game measures (Lichess accuracy etc.) apply.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import chess.pgn

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from make_games import Bot, replay  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--hl", required=True)
    ap.add_argument("--ratings", default="100,300,500,2200,2500,2800,3000")
    ap.add_argument("--games", type=int, default=8)
    ap.add_argument("--tc", default="300+0")
    ap.add_argument("--out", type=Path, default=Path("reports/acc_extremes/selfplay.pgn"))
    args = ap.parse_args()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    bot = Bot("homo_ludens", args)
    with open(args.out, "w") as f:
        for r in (int(x) for x in args.ratings.split(",")):
            for i in range(args.games):
                g = chess.pgn.Game()
                g.headers.update({"WhiteElo": str(r), "BlackElo": str(r), "TimeControl": args.tc, "Site": f"selfplay/{r}-{i}"})
                print(replay(g, bot, r * 100 + i), file=f, end="\n\n")
            print(f"rating {r}: {args.games} games", file=sys.stderr)
    bot.close()


if __name__ == "__main__":
    main()
