"""Build the corpora the anti-cheat grader compares.

humans  real games sampled from a PGN, both players in the same rating band
bots    for every human game, each bot replays the same first OPENING_PLIES
        plies and then plays the rest of the game itself, each side at the
        rating of the human it replaces. Same openings and ratings, so any
        difference the detector finds comes from how the moves are chosen.

Bots: homo_ludens (human mode), maia (rating-matched Maia-1 via lc0, 1 node),
engine (Homo Ludens at full strength, a known non-human control).

Writes one PGN per corpus. Bot games carry each move's simulated think time
as a [%emt] comment when the engine reports one.
"""

from __future__ import annotations

import argparse
import random
import sys
from pathlib import Path

import chess
import chess.pgn

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from uci_client import UciEngine  # noqa: E402

OPENING_PLIES = 8
MAIA_LEVELS = [1100, 1200, 1300, 1400, 1500, 1600, 1700, 1800, 1900]


def sample_human_games(pgn: Path, bands: list[int], per_band: int, width: int, seed: int) -> list[chess.pgn.Game]:
    rng = random.Random(seed)
    pools: dict[int, list] = {b: [] for b in bands}
    with open(pgn, encoding="utf-8", errors="replace") as f:
        while (g := chess.pgn.read_game(f)) is not None:
            try:
                we, be = int(g.headers["WhiteElo"]), int(g.headers["BlackElo"])
            except (KeyError, ValueError):
                continue
            band = (we // width) * width
            if band not in pools or (be // width) * width != band:
                continue
            if g.end().ply() < 36:  # Lichess assesses games of 36+ plies only
                continue
            pools[band].append(g)
    out = []
    for b in bands:
        rng.shuffle(pools[b])
        out += pools[b][:per_band]
        print(f"band {b}: {len(pools[b])} eligible, using {min(per_band, len(pools[b]))}", file=sys.stderr)
    return out


class Bot:
    def __init__(self, kind: str, args):
        self.kind = kind
        self.args = args
        self.engines: dict[int, UciEngine] = {}

    def _engine(self, key: int) -> UciEngine:
        if key not in self.engines:
            if self.kind == "homo_ludens":
                e = UciEngine([self.args.hl], {"UCI_LimitStrength": "true", "ShowThoughts": "false"})
            elif self.kind == "engine":
                e = UciEngine([self.args.hl], {"UCI_LimitStrength": "false"})
            else:
                w = Path(self.args.maia_dir) / f"maia-{key}.pb.gz"
                e = UciEngine([self.args.lc0, f"--weights={w}", "--backend=blas", "--threads=1"])
            self.engines[key] = e
        return self.engines[key]

    def move(self, start_fen: str, moves: list[str], elo: int, seed: int, tc: str = "300+0") -> tuple[str, float | None, bool]:
        if self.kind == "maia":
            level = min(MAIA_LEVELS, key=lambda l: abs(l - elo))
            r = self._engine(level).play(start_fen, moves, go="nodes 1")
            return r.move, None, False
        e = self._engine(0)
        if self.kind == "homo_ludens":
            e.set_option("UCI_Elo", elo)
            e.set_option("Seed", seed)
            e.set_option("SimTimeControl", tc)
            r = e.play(start_fen, moves, go="movetime 200")
            return r.move, (r.think_ms or 0) / 1000, "resign" in r.trace
        r = e.play(start_fen, moves, go=f"movetime {self.args.engine_ms}")
        return r.move, None, False

    def close(self):
        for e in self.engines.values():
            e.close()


def replay(human: chess.pgn.Game, bot: Bot, game_no: int) -> chess.pgn.Game:
    h = human.headers
    elo = {chess.WHITE: int(h["WhiteElo"]), chess.BLACK: int(h["BlackElo"])}
    board = human.board()
    start_fen = board.fen()
    moves: list[str] = []
    out = chess.pgn.Game()
    for k in ("WhiteElo", "BlackElo", "TimeControl"):
        if k in h:
            out.headers[k] = h[k]
    out.headers["Event"] = f"{bot.kind} replay"
    out.headers["Site"] = f"{bot.kind}/{h.get('Site', '').rsplit('/', 1)[-1]}"
    out.headers["White"] = f"{bot.kind}{elo[chess.WHITE]}"
    out.headers["Black"] = f"{bot.kind}{elo[chess.BLACK]}"
    node = out
    for i, m in enumerate(human.mainline_moves()):
        if i >= OPENING_PLIES:
            break
        node = node.add_variation(m)
        moves.append(m.uci())
        board.push(m)
    while not board.is_game_over(claim_draw=True) and board.ply() < 200:
        uci, secs, resign = bot.move(start_fen, moves, elo[board.turn], seed=game_no * 1000 + board.ply(), tc=h.get("TimeControl", "300+0"))
        if resign:
            out.headers["Termination"] = "Normal"
            out.headers["Result"] = "0-1" if board.turn == chess.WHITE else "1-0"
            return out
        m = chess.Move.from_uci(uci)
        if m not in board.legal_moves:
            print(f"illegal move {uci} from {bot.kind}", file=sys.stderr)
            break
        node = node.add_variation(m)
        if secs is not None:
            node.comment = f"[%emt {secs:.1f}]"
        moves.append(uci)
        board.push(m)
    out.headers["Result"] = board.result(claim_draw=True)
    out.headers["Termination"] = "Normal"
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pgn", required=True, type=Path)
    ap.add_argument("--hl", required=True)
    ap.add_argument("--lc0")
    ap.add_argument("--maia-dir")
    ap.add_argument("--bands", default="1000,1200,1400,1600,1800,2000")
    ap.add_argument("--per-band", type=int, default=25)
    ap.add_argument("--band-width", type=int, default=200)
    ap.add_argument("--bots", default="homo_ludens,maia,engine")
    ap.add_argument("--engine-games", type=int, default=60, help="cap for the slow full-strength control")
    ap.add_argument("--engine-ms", type=int, default=100)
    ap.add_argument("--seed", type=int, default=3)
    ap.add_argument("--out-dir", type=Path, default=Path("reports/anticheat"))
    args = ap.parse_args()
    args.out_dir.mkdir(parents=True, exist_ok=True)

    humans = sample_human_games(args.pgn, [int(b) for b in args.bands.split(",")], args.per_band, args.band_width, args.seed)
    with open(args.out_dir / "humans.pgn", "w") as f:
        for g in humans:
            print(g, file=f, end="\n\n")

    for kind in args.bots.split(","):
        bot = Bot(kind, args)
        subset = humans
        if kind == "engine":
            subset = random.Random(args.seed).sample(humans, min(args.engine_games, len(humans)))
        with open(args.out_dir / f"{kind}.pgn", "w") as f:
            for i, h in enumerate(subset):
                print(replay(h, bot, i), file=f, end="\n\n")
                if (i + 1) % 25 == 0:
                    print(f"{kind}: {i + 1}/{len(subset)} games", file=sys.stderr)
        bot.close()


if __name__ == "__main__":
    main()
