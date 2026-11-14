"""Who predicts the human move best, on the same positions.

Positions are stratified by rating band. Models:
  homo_ludens        human mode at the mover's rating, one move
  homo_ludens_mode   most common move over N seeds
  hl_engine          full strength, fixed depth
  maia_matched       nearest maia-1 net (1100-1900), 1 node
  maia_1500          maia-1500 for everyone

Engines get the move history, not just a fen (maia looks at the last 8 positions).
Writes <out>.json, <out>.md and <out>_positions.csv.

  python harness/compare.py --pgn data/lichess_db_standard_rated_2013-01.pgn \
      --hl ./target/release/homo_ludens --lc0 ../dl/lc0/build/release/lc0 \
      --maia-dir ../dl/maia/maia_weights --per-band 700 --out reports/baseline_v0
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import random
import sys
import time
from collections import defaultdict
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from positions import Position, band_of, iter_positions  # noqa: E402
from uci_client import UciEngine  # noqa: E402

MAIA_LEVELS = [1100, 1200, 1300, 1400, 1500, 1600, 1700, 1800, 1900]


def sample_positions(pgn: Path, per_band: int, band_width: int, min_ply: int, seed: int) -> list[Position]:
    """Reservoir-sample up to `per_band` positions in each rating band,
    at most one position per game and side so no single game dominates."""
    rng = random.Random(seed)
    reservoirs: dict[str, list[Position]] = defaultdict(list)
    seen: dict[str, int] = defaultdict(int)
    by_game: dict[tuple[str, int], list[Position]] = defaultdict(list)
    with open(pgn, encoding="utf-8", errors="replace") as f:
        for pos in iter_positions(f, min_ply=min_ply, min_clock_s=0, sample_rate=1.0, seed=seed):
            by_game[(pos.game_id, pos.ply % 2)].append(pos)
    for _, plist in by_game.items():
        pos = rng.choice(plist)
        band = band_of(pos.elo, band_width)
        seen[band] += 1
        r = reservoirs[band]
        if len(r) < per_band:
            r.append(pos)
        else:
            j = rng.randrange(seen[band])
            if j < per_band:
                r[j] = pos
    out = [p for r in reservoirs.values() for p in r]
    rng.shuffle(out)
    return out


def _go(engine: UciEngine, pos: Position, go: str) -> str:
    return engine.play(pos.start_fen, list(pos.moves_before), go=go).move


def run_model(job: tuple) -> tuple[str, list[str], float]:
    name, args, positions = job
    t0 = time.time()
    moves: list[str] = []
    if name == "homo_ludens":
        eng = UciEngine([args["hl"]], {"UCI_LimitStrength": "true", "ShowThoughts": "false", "Seed": str(args["seed"])})
        for i, pos in enumerate(positions):
            eng.set_option("UCI_Elo", pos.elo)
            moves.append(_go(eng, pos, "movetime 200"))
        eng.close()
    elif name == "homo_ludens_mode":
        # The persona's most likely move: the most frequent choice over N seeds.
        from collections import Counter

        eng = UciEngine([args["hl"]], {"UCI_LimitStrength": "true", "ShowThoughts": "false"})
        for pos in positions:
            eng.set_option("UCI_Elo", pos.elo)
            votes: Counter = Counter()
            for s in range(args["mode_samples"]):
                eng.set_option("Seed", 1000 + s)
                votes[_go(eng, pos, "movetime 200")] += 1
            moves.append(votes.most_common(1)[0][0])
        eng.close()
    elif name == "hl_engine":
        eng = UciEngine([args["hl"]], {"UCI_LimitStrength": "false"})
        for pos in positions:
            moves.append(_go(eng, pos, f"depth {args['engine_depth']}"))
        eng.close()
    elif name in ("maia_matched", "maia_1500"):
        engines: dict[int, UciEngine] = {}

        def maia(level: int) -> UciEngine:
            if level not in engines:
                w = Path(args["maia_dir"]) / f"maia-{level}.pb.gz"
                engines[level] = UciEngine([args["lc0"], f"--weights={w}", "--backend=blas", "--threads=1"])
            return engines[level]

        for pos in positions:
            if name == "maia_1500":
                level = 1500
            else:
                level = min(MAIA_LEVELS, key=lambda l: abs(l - pos.elo))
            moves.append(_go(maia(level), pos, "nodes 1"))
        for e in engines.values():
            e.close()
    else:
        raise ValueError(name)
    return name, moves, time.time() - t0


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (c - h, c + h)


def paired_diff_ci(a: list[int], b: list[int], z: float = 1.96) -> tuple[float, float, float]:
    """Difference in accuracy (a - b) on the same positions, normal approximation."""
    n = len(a)
    d = [x - y for x, y in zip(a, b)]
    mean = sum(d) / n
    var = sum((x - mean) ** 2 for x in d) / max(1, n - 1)
    h = z * math.sqrt(var / n)
    return mean, mean - h, mean + h


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--pgn", required=True, type=Path)
    ap.add_argument("--hl", required=True)
    ap.add_argument("--lc0")
    ap.add_argument("--maia-dir")
    ap.add_argument("--per-band", type=int, default=700)
    ap.add_argument("--band-width", type=int, default=200)
    ap.add_argument("--min-ply", type=int, default=10)
    ap.add_argument("--engine-depth", type=int, default=6)
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--mode-samples", type=int, default=8)
    ap.add_argument("--workers", type=int, default=2)
    ap.add_argument("--out", type=Path, default=Path("reports/compare"))
    args = ap.parse_args()

    t0 = time.time()
    positions = sample_positions(args.pgn, args.per_band, args.band_width, args.min_ply, args.seed)
    print(f"sampled {len(positions)} positions in {time.time() - t0:.0f}s", file=sys.stderr)

    models = ["homo_ludens", "homo_ludens_mode", "hl_engine"]
    if args.lc0 and args.maia_dir:
        models += ["maia_matched", "maia_1500"]
    shared = {"hl": args.hl, "lc0": args.lc0, "maia_dir": args.maia_dir, "seed": args.seed, "engine_depth": args.engine_depth, "mode_samples": args.mode_samples}
    preds: dict[str, list[str]] = {}
    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        for name, moves, secs in ex.map(run_model, [(m, shared, positions) for m in models]):
            preds[name] = moves
            print(f"{name}: {secs:.0f}s", file=sys.stderr)

    hits = {m: [int(p == pos.human_move) for p, pos in zip(preds[m], positions)] for m in models}
    bands = sorted({band_of(p.elo, args.band_width) for p in positions}, key=lambda b: int(b.split("-")[0]))
    table = []
    for band in bands:
        idx = [i for i, p in enumerate(positions) if band_of(p.elo, args.band_width) == band]
        row = {"band": band, "positions": len(idx)}
        for m in models:
            k = sum(hits[m][i] for i in idx)
            row[m] = {"accuracy": k / len(idx), "ci95": wilson(k, len(idx))}
        table.append(row)
    overall = {m: {"accuracy": sum(hits[m]) / len(positions), "ci95": wilson(sum(hits[m]), len(positions))} for m in models}
    diffs = {}
    for m in models:
        if m != "homo_ludens_mode":
            mean, lo, hi = paired_diff_ci(hits["homo_ludens_mode"], hits[m])
            diffs[f"homo_ludens_mode - {m}"] = {"diff": mean, "ci95": [lo, hi]}

    report = {
        "pgn": str(args.pgn),
        "positions": len(positions),
        "models": models,
        "overall": overall,
        "paired_differences": diffs,
        "bands": table,
        "seconds": round(time.time() - t0),
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    Path(f"{args.out}.json").write_text(json.dumps(report, indent=2))

    with open(f"{args.out}_positions.csv", "w", newline="") as f:
        w = csv.writer(f)
        w.writerow(["game_id", "ply", "elo", "time_control", "fen", "human_move"] + models)
        for i, p in enumerate(positions):
            w.writerow([p.game_id, p.ply, p.elo, p.time_control, p.fen, p.human_move] + [preds[m][i] for m in models])

    lines = [f"Positions: {len(positions)} from `{args.pgn.name}`\n"]
    head = "| Rating band | n | " + " | ".join(models) + " |"
    lines += [head, "| --- | ---: |" + " ---: |" * len(models)]
    for r in table:
        lines.append(f"| {r['band']} | {r['positions']} | " + " | ".join(f"{r[m]['accuracy']:.1%}" for m in models) + " |")
    lines.append("| **All** | " + str(len(positions)) + " | " + " | ".join(f"**{overall[m]['accuracy']:.1%}**" for m in models) + " |")
    lines.append("")
    for k, v in diffs.items():
        lines.append(f"- {k}: {v['diff']:+.1%} (95% CI {v['ci95'][0]:+.1%} to {v['ci95'][1]:+.1%})")
    md = "\n".join(lines) + "\n"
    Path(f"{args.out}.md").write_text(md)
    print(md)


if __name__ == "__main__":
    main()
