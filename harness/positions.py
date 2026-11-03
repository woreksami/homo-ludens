"""Turn human games (PGN, e.g. the Lichess open database) into test positions.

Each test position records the board before a human move, the move the human
actually played, the mover's rating, and the human's think time from the
`[%clk h:mm:ss]` comments that Lichess writes on every move.
"""

from __future__ import annotations

import io
import random
import re
from dataclasses import dataclass
from typing import Iterator

import chess
import chess.pgn

CLK = re.compile(r"\[%clk (\d+):(\d+):(\d+(?:\.\d+)?)\]")


@dataclass
class Position:
    fen: str
    human_move: str
    elo: int
    think_s: float | None
    clock_s: float | None
    ply: int
    game_id: str
    time_control: str


def _clock(comment: str) -> float | None:
    m = CLK.search(comment or "")
    if not m:
        return None
    h, mi, s = m.groups()
    return int(h) * 3600 + int(mi) * 60 + float(s)


def _increment(tc: str) -> float:
    try:
        return float(tc.split("+")[1])
    except (IndexError, ValueError):
        return 0.0


def iter_positions(
    pgn_text_or_file,
    min_ply: int = 10,
    min_clock_s: float = 30.0,
    sample_rate: float = 1.0,
    seed: int = 0,
) -> Iterator[Position]:
    """Yield test positions from a PGN stream.

    Skips the opening (memorised theory says little about thinking), and moves
    made with under `min_clock_s` on the clock (time-scramble noise), following
    the Maia papers' filtering.
    """
    rng = random.Random(seed)
    stream = io.StringIO(pgn_text_or_file) if isinstance(pgn_text_or_file, str) else pgn_text_or_file
    while True:
        game = chess.pgn.read_game(stream)
        if game is None:
            return
        h = game.headers
        try:
            elos = {chess.WHITE: int(h["WhiteElo"]), chess.BLACK: int(h["BlackElo"])}
        except (KeyError, ValueError):
            continue
        tc = h.get("TimeControl", "-")
        inc = _increment(tc)
        gid = h.get("Site", "").rsplit("/", 1)[-1] or h.get("Event", "game")
        board = game.board()
        last_clock = {chess.WHITE: None, chess.BLACK: None}
        for ply, node in enumerate(game.mainline()):
            mover = board.turn
            clk = _clock(node.comment)
            prev = last_clock[mover]
            think = None
            if clk is not None and prev is not None:
                think = max(0.0, prev - clk + inc)
            if (
                ply >= min_ply
                and (clk is None or clk >= min_clock_s)
                and rng.random() < sample_rate
            ):
                yield Position(
                    fen=board.fen(),
                    human_move=node.move.uci(),
                    elo=elos[mover],
                    think_s=think,
                    clock_s=clk,
                    ply=ply,
                    game_id=gid,
                    time_control=tc,
                )
            last_clock[mover] = clk
            board.push(node.move)


def band_of(elo: int, width: int = 200) -> str:
    lo = (elo // width) * width
    return f"{lo}-{lo + width - 1}"
