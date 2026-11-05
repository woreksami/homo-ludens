"""Analyse every position of every corpus with Stockfish, as Lichess does.

For each position (from ply OPENING_PLIES on) stores Stockfish's top lines
(MultiPV) with centipawn scores from the side to move's point of view.
Results are cached in a JSONL keyed by FEN, so re-runs and new corpora only
analyse new positions.
"""

from __future__ import annotations

import argparse
import json
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import chess
import chess.engine
import chess.pgn

MATE_CP = 2000  # mate scores are clipped here, like Lichess's ±1000 cp cap but wider for ranking


def positions_of(pgn: Path, start_ply: int) -> list[str]:
    out = []
    with open(pgn, encoding="utf-8", errors="replace") as f:
        while (g := chess.pgn.read_game(f)) is not None:
            b = g.board()
            for i, m in enumerate(g.mainline_moves()):
                if i >= start_ply:
                    out.append(b.fen())
                b.push(m)
            out.append(b.fen())  # final position, needed for the last move's "after" eval
    return out


def _worker(job):
    sf_path, fens, nodes, multipv = job
    eng = chess.engine.SimpleEngine.popen_uci(sf_path, timeout=120)
    eng.configure({"Threads": 1, "Hash": 64})
    out = []
    for fen in fens:
        b = chess.Board(fen)
        if b.is_game_over():
            out.append({"fen": fen, "over": True, "lines": []})
            continue
        infos = eng.analyse(b, chess.engine.Limit(nodes=nodes), multipv=multipv)
        lines = []
        for info in infos:
            if "pv" not in info:
                continue
            cp = info["score"].pov(b.turn).score(mate_score=MATE_CP * 10)
            lines.append([info["pv"][0].uci(), max(-MATE_CP, min(MATE_CP, cp))])
        out.append({"fen": fen, "over": False, "lines": lines})
    eng.quit()
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--stockfish", required=True)
    ap.add_argument("--pgns", nargs="+", type=Path, required=True)
    ap.add_argument("--cache", type=Path, default=None, help="default: <dir of first pgn>/analysis_<nodes>.jsonl")
    ap.add_argument("--nodes", type=int, default=10_000)
    ap.add_argument("--multipv", type=int, default=3)
    ap.add_argument("--start-ply", type=int, default=8)
    ap.add_argument("--workers", type=int, default=2)
    args = ap.parse_args()
    if args.cache is None:
        args.cache = args.pgns[0].parent / f"analysis_{args.nodes}.jsonl"

    done = set()
    if args.cache.exists():
        for line in open(args.cache):
            done.add(json.loads(line)["fen"])
    todo = []
    for p in args.pgns:
        for fen in positions_of(p, args.start_ply):
            if fen not in done:
                done.add(fen)
                todo.append(fen)
    print(f"{len(todo)} new positions to analyse", file=sys.stderr)
    chunk = 200
    jobs = [(args.stockfish, todo[i:i + chunk], args.nodes, args.multipv) for i in range(0, len(todo), chunk)]
    n = 0
    with open(args.cache, "a") as cache, ProcessPoolExecutor(max_workers=args.workers) as ex:
        for res in ex.map(_worker, jobs):
            for r in res:
                cache.write(json.dumps(r) + "\n")
            cache.flush()
            n += len(res)
            print(f"  {n}/{len(todo)}", file=sys.stderr)


if __name__ == "__main__":
    main()
