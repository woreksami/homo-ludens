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
        # Resigning is the operator's decision on a real account, so test games
        # play on to mate, a flag, a draw or the ply cap unless asked otherwise.
        self.args_allow_resign = getattr(args, "allow_resign", False)

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

    def move(self, start_fen: str, moves: list[str], elo: int, seed: int, tc: str = "300+0", clock: str = "") -> tuple[str, float | None, bool]:
        if self.kind == "maia":
            level = min(MAIA_LEVELS, key=lambda l: abs(l - elo))
            r = self._engine(level).play(start_fen, moves, go="nodes 1")
            return r.move, None, False
        e = self._engine(0)
        if self.kind == "homo_ludens":
            e.set_option("UCI_Elo", elo)
            e.set_option("Seed", seed)
            e.set_option("SimTimeControl", tc)
            r = e.play(start_fen, moves, go=f"movetime {getattr(self.args, 'hl_ms', 200)} {clock}".strip())
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
    # Simulated clocks (Homo Ludens only): the replayed opening is charged about
    # a second a move, then each move's simulated think time, plus increment.
    tc = h.get("TimeControl", "300+0")
    try:
        base, inc = (float(x) for x in tc.split("+"))
    except ValueError:
        base, inc = 0.0, 0.0
    clocks = {chess.WHITE: base - OPENING_PLIES / 2, chess.BLACK: base - OPENING_PLIES / 2} if bot.kind == "homo_ludens" and base > 0 else None
    while not board.is_game_over(claim_draw=True) and board.ply() < 200:
        clock = ""
        if clocks:
            clock = (f"wtime {int(clocks[chess.WHITE] * 1000)} btime {int(clocks[chess.BLACK] * 1000)} "
                     f"winc {int(inc * 1000)} binc {int(inc * 1000)}")
        uci, secs, resign = bot.move(start_fen, moves, elo[board.turn], seed=game_no * 1000 + board.ply(), tc=tc, clock=clock)
        if clocks and secs is not None:
            clocks[board.turn] -= secs
            if clocks[board.turn] <= 0:
                out.headers["Termination"] = "Time forfeit"
                out.headers["Result"] = "1/2-1/2" if board.has_insufficient_material(not board.turn) else ("0-1" if board.turn == chess.WHITE else "1-0")
                return out
            clocks[board.turn] += inc
        if resign and bot.args_allow_resign:
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


_BOT = None


def _init_worker(kind, args):
    global _BOT
    _BOT = Bot(kind, args)


def _play(job):
    import io
    i, text = job
    return str(replay(chess.pgn.read_game(io.StringIO(text)), _BOT, i))


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
    ap.add_argument("--hl-ms", type=int, default=200, help="real compute per Homo Ludens move")
    ap.add_argument("--jobs", type=int, default=1, help="parallel Homo Ludens games")
    ap.add_argument("--seed", type=int, default=3)
    ap.add_argument("--allow-resign", action="store_true", help="let the engine resign (off: the operator resigns on real accounts)")
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
        if kind == "homo_ludens" and args.jobs > 1:
            bot.close()
            from multiprocessing import Pool
            with Pool(args.jobs, initializer=_init_worker, initargs=(kind, args)) as pool, open(args.out_dir / f"{kind}.pgn", "w") as f:
                for i, g in enumerate(pool.imap(_play, [(i, str(h)) for i, h in enumerate(subset)])):
                    print(g, file=f, end="\n\n")
                    if (i + 1) % 25 == 0:
                        print(f"{kind}: {i + 1}/{len(subset)} games", file=sys.stderr)
            continue
        with open(args.out_dir / f"{kind}.pgn", "w") as f:
            for i, h in enumerate(subset):
                print(replay(h, bot, i), file=f, end="\n\n")
                if (i + 1) % 25 == 0:
                    print(f"{kind}: {i + 1}/{len(subset)} games", file=sys.stderr)
        bot.close()


if __name__ == "__main__":
    main()
