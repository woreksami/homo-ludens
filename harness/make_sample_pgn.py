"""Write a small SYNTHETIC PGN for testing the pipeline end to end.

The games are Homo Ludens personas playing each other, with Lichess-style
headers and [%clk] comments built from the simulated think times. They are
for smoke-testing the harness only: grading the engine against its own games
measures nothing about humans. Use real Lichess games (fetch_lichess.sh) for
any result you report.
"""

from __future__ import annotations

import argparse
import random
import sys
from pathlib import Path

import chess
import chess.pgn

sys.path.insert(0, str(Path(__file__).parent))
from uci_client import UciEngine  # noqa: E402


def fmt_clock(s: float) -> str:
    t = max(0, round(s * 10))  # tenths of a second, so 59.96 never prints as 60.0
    h, rem = divmod(t, 36000)
    m, rem = divmod(rem, 600)
    return f"{h}:{m:02d}:{rem // 10:02d}.{rem % 10}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--engine", required=True)
    ap.add_argument("--games", type=int, default=12)
    ap.add_argument("--out", type=Path, default=Path("data/sample_synthetic.pgn"))
    ap.add_argument("--seed", type=int, default=7)
    args = ap.parse_args()
    rng = random.Random(args.seed)
    args.out.parent.mkdir(parents=True, exist_ok=True)

    with UciEngine(args.engine.split(), {"ShowThoughts": "false"}) as eng, open(args.out, "w") as out:
        for g in range(args.games):
            elo = {chess.WHITE: rng.randrange(600, 2400, 50), chess.BLACK: 0}
            elo[chess.BLACK] = max(400, min(2600, elo[chess.WHITE] + rng.randrange(-200, 201, 50)))
            game = chess.pgn.Game()
            game.headers.update({
                "Event": "Synthetic persona game (pipeline test only)",
                "Site": f"https://example.invalid/synthetic{g}",
                "White": f"persona{elo[chess.WHITE]}",
                "Black": f"persona{elo[chess.BLACK]}",
                "WhiteElo": str(elo[chess.WHITE]),
                "BlackElo": str(elo[chess.BLACK]),
                "TimeControl": "600+0",
            })
            clock = {chess.WHITE: 600.0, chess.BLACK: 600.0}
            board = chess.Board()
            node = game
            eng.new_game()
            eng.set_option("Seed", rng.randrange(1, 10**6))
            while not board.is_game_over(claim_draw=True) and board.ply() < 160:
                eng.set_option("UCI_Elo", elo[board.turn])
                reply = eng.play(board.fen(), go="movetime 50")
                think = (reply.think_ms or 1000) / 1000
                clock[board.turn] -= think
                move = chess.Move.from_uci(reply.move)
                node = node.add_variation(move)
                node.comment = f"[%clk {fmt_clock(clock[board.turn])}]"
                board.push(move)
            game.headers["Result"] = board.result(claim_draw=True)
            print(game, file=out, end="\n\n")
            print(f"game {g + 1}: {elo[chess.WHITE]} vs {elo[chess.BLACK]} {game.headers['Result']}", file=sys.stderr)


if __name__ == "__main__":
    main()
