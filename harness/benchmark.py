"""Old move-matching benchmark (pre anti-cheat).

Gives the engine each test position with the mover's rating and checks:
- move match per rating band (the Maia number)
- spearman between our fake think time and the human's real one
- optionally, whether best-move rate goes up with rating (maia-2 coherence)

Works for any UCI engine, so maia/stockfish run through the same thing.

  python harness/benchmark.py --engine target/release/homo_ludens \
      --pgn data/lichess_db_standard_rated_2025-01.pgn --max-positions 5000
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
import sys
import time
from collections import defaultdict
from pathlib import Path

import chess

sys.path.insert(0, str(Path(__file__).parent))
from positions import Position, band_of, iter_positions  # noqa: E402
from uci_client import UciEngine  # noqa: E402


def spearman(xs: list[float], ys: list[float]) -> float | None:
    if len(xs) < 5:
        return None

    def ranks(v):
        order = sorted(range(len(v)), key=lambda i: v[i])
        r = [0.0] * len(v)
        i = 0
        while i < len(order):
            j = i
            while j + 1 < len(order) and v[order[j + 1]] == v[order[i]]:
                j += 1
            for k in range(i, j + 1):
                r[order[k]] = (i + j) / 2
            i = j + 1
        return r

    rx, ry = ranks(xs), ranks(ys)
    mx, my = statistics.fmean(rx), statistics.fmean(ry)
    num = sum((a - mx) * (b - my) for a, b in zip(rx, ry))
    den = math.sqrt(sum((a - mx) ** 2 for a in rx) * sum((b - my) ** 2 for b in ry))
    return num / den if den else None


def wilson(k: int, n: int, z: float = 1.96) -> tuple[float, float]:
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (c - h, c + h)


def run(args) -> dict:
    opts = {"UCI_LimitStrength": "true", "ShowThoughts": "false", "Seed": str(args.seed)}
    engine = UciEngine(args.engine.split(), opts)
    by_band: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    times_pred: list[float] = []
    times_human: list[float] = []
    examples: list[dict] = []
    n = 0
    t0 = time.time()

    with open(args.pgn, encoding="utf-8", errors="replace") as f:
        for pos in iter_positions(f, min_ply=args.min_ply, sample_rate=args.sample_rate, seed=args.seed):
            if args.min_elo and pos.elo < args.min_elo:
                continue
            if args.max_elo and pos.elo > args.max_elo:
                continue
            engine.set_option("UCI_Elo", pos.elo)
            reply = engine.play(pos.fen, go=args.go)
            hit = int(reply.move == pos.human_move)
            band = band_of(pos.elo, args.band_width)
            by_band[band][0] += hit
            by_band[band][1] += 1
            if reply.think_ms is not None and pos.think_s is not None:
                times_pred.append(reply.think_ms / 1000)
                times_human.append(pos.think_s)
            if len(examples) < 20 and not hit:
                examples.append({"fen": pos.fen, "elo": pos.elo, "human": pos.human_move, "engine": reply.move})
            n += 1
            if n % 250 == 0:
                print(f"  {n} positions, {time.time() - t0:.0f}s", file=sys.stderr)
            if n >= args.max_positions:
                break

    engine.close()
    bands = []
    for band in sorted(by_band, key=lambda b: int(b.split("-")[0])):
        k, total = by_band[band]
        lo, hi = wilson(k, total)
        bands.append({"band": band, "positions": total, "accuracy": k / total, "ci95": [lo, hi]})
    hits = sum(v[0] for v in by_band.values())
    return {
        "engine": args.engine,
        "pgn": str(args.pgn),
        "positions": n,
        "accuracy": hits / n if n else 0.0,
        "bands": bands,
        "think_time_spearman": spearman(times_pred, times_human),
        "think_time_pairs": len(times_pred),
        "sample_misses": examples,
        "seconds": round(time.time() - t0, 1),
    }


def coherence(args) -> dict:
    """Share of positions where P(engine-best move) rises with rating.

    A position counts as monotonic if the estimated probability of playing the
    reference move never drops by more than the sampling noise as Elo rises.
    """
    ref = UciEngine(args.reference.split(), {"UCI_LimitStrength": "false"})
    eng = UciEngine(args.engine.split(), {"UCI_LimitStrength": "true", "ShowThoughts": "false"})
    grid = list(range(args.coh_min, args.coh_max + 1, args.coh_step))
    mono, total, rows = 0, 0, []
    with open(args.pgn, encoding="utf-8", errors="replace") as f:
        for pos in iter_positions(f, min_ply=args.min_ply, sample_rate=args.sample_rate, seed=args.seed):
            best = ref.play(pos.fen, go="depth 10").move
            probs = []
            for elo in grid:
                eng.set_option("UCI_Elo", elo)
                k = 0
                for s in range(args.coh_samples):
                    eng.set_option("Seed", 1000 + s)
                    k += eng.play(pos.fen, go=args.go).move == best
                probs.append(k / args.coh_samples)
            slack = 1.0 / args.coh_samples
            is_mono = all(b >= a - slack for a, b in zip(probs, probs[1:]))
            mono += is_mono
            total += 1
            rows.append({"fen": pos.fen, "best": best, "p_best": probs})
            if total >= args.coh_positions:
                break
    ref.close()
    eng.close()
    return {"elo_grid": grid, "positions": total, "monotonic_share": mono / total if total else 0.0, "rows": rows}


def markdown(report: dict) -> str:
    out = [f"# Human-likeness report\n", f"Engine: `{report['engine']}`  ", f"Positions: {report['positions']}  ",
           f"Overall move-matching accuracy: **{report['accuracy']:.1%}**\n"]
    out.append("| Rating band | Positions | Accuracy | 95% CI |")
    out.append("| --- | ---: | ---: | --- |")
    for b in report["bands"]:
        out.append(f"| {b['band']} | {b['positions']} | {b['accuracy']:.1%} | {b['ci95'][0]:.1%} to {b['ci95'][1]:.1%} |")
    rho = report.get("think_time_spearman")
    out.append(f"\nThink-time Spearman correlation: {'n/a' if rho is None else f'{rho:.2f}'} "
               f"({report['think_time_pairs']} pairs)")
    if "coherence" in report:
        c = report["coherence"]
        out.append(f"\nCoherence (monotonic share over Elo {c['elo_grid'][0]}..{c['elo_grid'][-1]}): "
                   f"**{c['monotonic_share']:.0%}** of {c['positions']} positions")
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--engine", required=True, help="command line of a UCI engine")
    ap.add_argument("--pgn", required=True, type=Path)
    ap.add_argument("--max-positions", type=int, default=2000)
    ap.add_argument("--sample-rate", type=float, default=0.2, help="fraction of eligible positions to test")
    ap.add_argument("--min-ply", type=int, default=10)
    ap.add_argument("--min-elo", type=int)
    ap.add_argument("--max-elo", type=int)
    ap.add_argument("--band-width", type=int, default=200)
    ap.add_argument("--go", default="movetime 100", help="UCI go arguments per position")
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--out", type=Path, default=Path("report"))
    ap.add_argument("--coherence", action="store_true", help="also run the monotonicity test")
    ap.add_argument("--reference", help="UCI engine for the 'best move' in coherence (e.g. stockfish)")
    ap.add_argument("--coh-positions", type=int, default=50)
    ap.add_argument("--coh-samples", type=int, default=8)
    ap.add_argument("--coh-min", type=int, default=600)
    ap.add_argument("--coh-max", type=int, default=2400)
    ap.add_argument("--coh-step", type=int, default=300)
    args = ap.parse_args()

    report = run(args)
    if args.coherence:
        args.reference = args.reference or args.engine
        report["coherence"] = coherence(args)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    Path(f"{args.out}.json").write_text(json.dumps(report, indent=2))
    Path(f"{args.out}.md").write_text(markdown(report))
    print(markdown(report))


if __name__ == "__main__":
    main()
