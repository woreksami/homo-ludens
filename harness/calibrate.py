"""Calibrate Homo Ludens's strength dial against real players, move by move.

Samples positions from real games, asks Homo Ludens for a move at several
UCI_Elo settings, and scores every move (human and bot) with Stockfish.
Prints, per rating band, the humans' average centipawn loss and blunder
rate next to Homo Ludens's at each setting, so the setting that plays like
each band can be read off. Much faster than replaying whole games.
"""

from __future__ import annotations

import argparse
import json
import random
import statistics
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import chess
import chess.engine
import chess.pgn

sys.path.insert(0, str(Path(__file__).resolve().parent))
from uci_client import UciEngine  # noqa: E402

CLIP = 1000


def sample(pgn: Path, per_band: int, width: int, seed: int):
    rng = random.Random(seed)
    pools = {}
    with open(pgn, encoding="utf-8", errors="replace") as f:
        while (g := chess.pgn.read_game(f)) is not None:
            elo = {chess.WHITE: int(g.headers["WhiteElo"]), chess.BLACK: int(g.headers["BlackElo"])}
            b = g.board()
            moves = []
            for i, m in enumerate(g.mainline_moves()):
                if i >= 8:
                    pools.setdefault(elo[b.turn] // width * width, []).append((b.fen(), list(moves), m.uci(), elo[b.turn]))
                moves.append(m.uci())
                b.push(m)
    start = chess.STARTING_FEN
    out = []
    for band in sorted(pools):
        rng.shuffle(pools[band])
        out += [(band, start, *x[1:]) for x in pools[band][:per_band]]
    return out


class Scorer:
    """Stockfish centipawn loss of a move, cached."""

    def __init__(self, path: str, nodes: int, cache: Path):
        self.eng = chess.engine.SimpleEngine.popen_uci(path, timeout=120)
        self.eng.configure({"Threads": 1, "Hash": 64})
        self.nodes = nodes
        self.cache_path = cache
        self.cache = json.loads(cache.read_text()) if cache.exists() else {}

    def _cp(self, b: chess.Board, moves=None) -> int:
        info = self.eng.analyse(b, chess.engine.Limit(nodes=self.nodes), root_moves=moves)
        return max(-CLIP, min(CLIP, info["score"].pov(b.turn).score(mate_score=CLIP * 10)))

    def loss(self, fen: str, uci: str) -> int:
        k = f"{fen}|{uci}"
        if k not in self.cache:
            b = chess.Board(fen)
            best = self.cache.setdefault(f"{fen}|*", self._cp(b))
            self.cache[k] = max(0, best - self._cp(b, [chess.Move.from_uci(uci)]))
        return self.cache[k]

    def save(self):
        self.cache_path.write_text(json.dumps(self.cache))


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--pgn", type=Path, required=True)
    ap.add_argument("--hl", required=True)
    ap.add_argument("--stockfish", required=True)
    ap.add_argument("--settings", default="800,1200,1600,2000,2400")
    ap.add_argument("--per-band", type=int, default=60)
    ap.add_argument("--nodes", type=int, default=30000)
    ap.add_argument("--movetime", type=int, default=200, help="real compute per move, ms")
    ap.add_argument("--raw", action="store_true", help="settings are internal skill, not calibrated ratings")
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--cache", type=Path, default=Path("reports/calib_cache.json"))
    ap.add_argument("--out", type=Path, default=Path("reports/calibration.json"))
    args = ap.parse_args()
    args.cache.parent.mkdir(parents=True, exist_ok=True)

    pos = sample(args.pgn, args.per_band, 200, 7)
    settings = [int(s) for s in args.settings.split(",")]
    print(f"{len(pos)} positions x {len(settings)} settings", file=sys.stderr)

    def hl_moves(setting: int):
        e = UciEngine([args.hl], {"UCI_LimitStrength": "true", "ShowThoughts": "false", "UCI_Elo": setting, "RawSkill": "true" if args.raw else "false"})
        out = []
        for i, (_, start, moves, _, _) in enumerate(pos):
            e.set_option("Seed", 1 + i)
            out.append(e.play(start, moves, go=f"movetime {args.movetime}").move)
        e.close()
        return setting, out

    moves_path = args.out.with_suffix(".moves.json")
    bot = {int(k): v for k, v in json.loads(moves_path.read_text()).items()} if moves_path.exists() else {}
    todo = [s for s in settings if s not in bot or len(bot[s]) != len(pos)]
    with ThreadPoolExecutor(args.workers) as ex:
        bot.update(dict(ex.map(hl_moves, todo)))
    moves_path.write_text(json.dumps(bot))

    sc = Scorer(args.stockfish, args.nodes, args.cache)
    rows = []
    for i, (band, start, moves, human, elo) in enumerate(pos):
        b = chess.Board(start)
        for m in moves:
            b.push_uci(m)
        fen = b.fen()
        r = {"band": band, "human": sc.loss(fen, human)}
        for s in settings:
            r[s] = sc.loss(fen, bot[s][i])
        rows.append(r)
        if i % 100 == 99:
            sc.save()
            print(f"  scored {i + 1}/{len(pos)}", file=sys.stderr)
    sc.save()
    sc.eng.quit()

    def stats(xs):
        return {"acpl": round(statistics.fmean(xs), 1), "blunder": round(sum(x >= 200 for x in xs) / len(xs), 3)}

    table = {}
    for band in sorted({r["band"] for r in rows}):
        rs = [r for r in rows if r["band"] == band]
        table[band] = {"human": stats([r["human"] for r in rs]), **{s: stats([r[s] for r in rs]) for s in settings}}
    args.out.write_text(json.dumps(table, indent=1))
    print("band  human(acpl/blunder)  " + "  ".join(f"HL@{s}" for s in settings))
    for band, t in table.items():
        print(f"{band:5} {t['human']['acpl']:6} {t['human']['blunder']:5}   " + "  ".join(f"{t[s]['acpl']:6}/{t[s]['blunder']:.2f}" for s in settings))
    allh = [r["human"] for r in rows]
    print("all humans", stats(allh), {s: stats([r[s] for r in rows]) for s in settings})


if __name__ == "__main__":
    main()
