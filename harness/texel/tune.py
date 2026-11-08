"""Texel tuning: fit the evaluation's weights to game results, then write src/eval_params.rs."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import features as F  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]


def old_tables():
    """Starting point: the hand-written tables in the previous src/eval.rs, a1 = 0."""
    src = (ROOT / "harness/texel/eval_v1_tables.rs").read_text()
    t = {}
    for name in ["PST_PAWN", "PST_KNIGHT", "PST_BISHOP", "PST_ROOK", "PST_QUEEN", "PST_KING_MG", "PST_KING_EG"]:
        body = re.search(name + r": \[i32; 64\] = \[(.*?)\];", src, re.S).group(1)
        vals = [int(v) for v in re.findall(r"-?\d+", body)]
        # written rank 8 first -> a1 = 0
        t[name] = [vals[(7 - (s // 8)) * 8 + s % 8] for s in range(64)]
    return t


def initial():
    mg = np.zeros(F.N_TERMS)
    eg = np.zeros(F.N_TERMS)
    for i, v in enumerate([100, 320, 330, 500, 900]):
        mg[F.MAT + i] = eg[F.MAT + i] = v
    t = old_tables()
    for k, name in enumerate(["PST_PAWN", "PST_KNIGHT", "PST_BISHOP", "PST_ROOK", "PST_QUEEN"]):
        mg[F.PST + k * 64:F.PST + (k + 1) * 64] = t[name]
        eg[F.PST + k * 64:F.PST + (k + 1) * 64] = t[name]
    mg[F.PST + 5 * 64:F.PST + 6 * 64] = t["PST_KING_MG"]
    eg[F.PST + 5 * 64:F.PST + 6 * 64] = t["PST_KING_EG"]
    for i, v in enumerate([4, 5, 2, 1]):
        mg[F.MOB + i] = eg[F.MOB + i] = v
    mg[F.DOUBLED] = eg[F.DOUBLED] = -15
    mg[F.ISOLATED] = eg[F.ISOLATED] = -12
    for i, v in enumerate([5, 10, 20, 35, 60, 100]):
        mg[F.PASSED + i] = v * 0.6
        eg[F.PASSED + i] = v * 1.2
    mg[F.SHIELD1], mg[F.SHIELD2], mg[F.KOPEN], mg[F.KATT] = 12, 6, -15, -4
    mg[F.ROOK_OPEN] = eg[F.ROOK_OPEN] = 20
    mg[F.ROOK_HALF] = eg[F.ROOK_HALF] = 10
    mg[F.BPAIR] = eg[F.BPAIR] = 30
    mg[F.PTHREAT] = eg[F.PTHREAT] = 30
    mg[F.OUTPOST], eg[F.OUTPOST] = 20, 10
    mg[F.SUPPORTED] = eg[F.SUPPORTED] = 5
    mg[F.ROOK7] = eg[F.ROOK7] = 15
    mg[F.TEMPO] = eg[F.TEMPO] = 10
    mg[F.CASTLE], eg[F.CASTLE] = 10, 0
    mg[F.UNDEV], eg[F.UNDEV] = -10, 0
    return mg, eg


def evals(X, ph, mg, eg):
    return (X @ mg) * ph + (X @ eg) * (1 - ph)


def loss(e, y, K):
    p = 1 / (1 + 10 ** (-K * e / 400))
    return float(np.mean((y - p) ** 2))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", default="reports/texel/data.npz")
    ap.add_argument("--iters", type=int, default=3000)
    ap.add_argument("--lr", type=float, default=2.0)
    ap.add_argument("--l2", type=float, default=1e-7, help="pull toward the starting weights")
    ap.add_argument("--out", default="src/eval_params.rs")
    ap.add_argument("--fixed-k", type=float, default=0.0, help="use this K instead of fitting it (SF-labelled data)")
    args = ap.parse_args()
    d = np.load(ROOT / args.data)
    X, ph, y = d["X"].astype(np.float64), d["ph"] / 24.0, d["y"].astype(np.float64)
    n = len(y)
    rng = np.random.default_rng(0)
    idx = rng.permutation(n)
    val, tr = idx[: n // 10], idx[n // 10:]
    mg0, eg0 = initial()
    mg, eg = mg0.copy(), eg0.copy()
    e0 = evals(X[tr], ph[tr], mg, eg)
    Ks = np.linspace(0.3, 2.0, 35)
    K = args.fixed_k or float(Ks[np.argmin([loss(e0, y[tr], k) for k in Ks])])
    print(f"{n} positions; K = {K:.2f}; start loss train {loss(e0, y[tr], K):.5f} val {loss(evals(X[val], ph[val], mg, eg), y[val], K):.5f}", file=sys.stderr)
    # The king has no material term; its PST must not drift as a whole.
    w = np.concatenate([mg, eg])
    w0 = w.copy()
    m = np.zeros_like(w)
    v = np.zeros_like(w)
    Xt, pht, yt = X[tr], ph[tr], y[tr]
    c = K * np.log(10) / 400
    for it in range(1, args.iters + 1):
        a, b = w[:F.N_TERMS], w[F.N_TERMS:]
        e = (Xt @ a) * pht + (Xt @ b) * (1 - pht)
        p = 1 / (1 + np.exp(-c * e))
        g_e = -2 * (yt - p) * p * (1 - p) * c / len(yt)
        ga = Xt.T @ (g_e * pht)
        gb = Xt.T @ (g_e * (1 - pht))
        g = np.concatenate([ga, gb]) + 2 * args.l2 * (w - w0)
        m = 0.9 * m + 0.1 * g
        v = 0.999 * v + 0.001 * g * g
        w -= args.lr * (m / (1 - 0.9 ** it)) / (np.sqrt(v / (1 - 0.999 ** it)) + 1e-12)
        if it % 250 == 0:
            a, b = w[:F.N_TERMS], w[F.N_TERMS:]
            print(f"  iter {it}: train {loss(evals(Xt, pht, a, b), yt, K):.5f} val {loss(evals(X[val], ph[val], a, b), y[val], K):.5f}", file=sys.stderr)
    mg, eg = np.rint(w[:F.N_TERMS]).astype(int), np.rint(w[F.N_TERMS:]).astype(int)
    write_rust(mg, eg, K, n, ROOT / args.out)
    print(f"material mg {mg[F.MAT:F.MAT+5].tolist()} eg {eg[F.MAT:F.MAT+5].tolist()}", file=sys.stderr)
    print(f"mobility mg {mg[F.MOB:F.MOB+4].tolist()} eg {eg[F.MOB:F.MOB+4].tolist()}", file=sys.stderr)
    print(f"passed mg {mg[F.PASSED:F.PASSED+6].tolist()} eg {eg[F.PASSED:F.PASSED+6].tolist()}", file=sys.stderr)
    print(f"doubled {mg[F.DOUBLED]},{eg[F.DOUBLED]} isolated {mg[F.ISOLATED]},{eg[F.ISOLATED]} shield {mg[F.SHIELD1]},{mg[F.SHIELD2]} kopen {mg[F.KOPEN]} katt {mg[F.KATT]},{eg[F.KATT]} rook {mg[F.ROOK_OPEN]},{mg[F.ROOK_HALF]} bpair {mg[F.BPAIR]},{eg[F.BPAIR]}", file=sys.stderr)


def write_rust(mg, eg, K, n, path):
    def arr(v):
        return "[" + ", ".join(str(int(x)) for x in v) + "]"

    def tab(v):
        rows = [", ".join(f"{int(x):4d}" for x in v[r * 8:(r + 1) * 8]) for r in range(8)]
        return "[\n" + "".join(f"        {row},\n" for row in rows) + "    ]"

    pst = lambda w: "[\n" + "".join(f"    {tab(w[F.PST + k * 64:F.PST + (k + 1) * 64])},\n" for k in range(6)) + "]"
    s = f"""//! Evaluation weights tuned on {n} positions from quiet positions against Stockfish evals (Texel's
//! method, K = {K:.2f}). Generated by harness/texel/tune.py; do not edit by hand.
//! Each pair is (middlegame, endgame). Square tables are indexed a1 = 0 from the
//! side's own point of view (Black's squares are mirrored).

pub const MAT_MG: [i32; 5] = {arr(mg[F.MAT:F.MAT + 5])};
pub const MAT_EG: [i32; 5] = {arr(eg[F.MAT:F.MAT + 5])};
#[rustfmt::skip]
pub const PST_MG: [[i32; 64]; 6] = {pst(mg)};
#[rustfmt::skip]
pub const PST_EG: [[i32; 64]; 6] = {pst(eg)};
pub const MOB_MG: [i32; 4] = {arr(mg[F.MOB:F.MOB + 4])};
pub const MOB_EG: [i32; 4] = {arr(eg[F.MOB:F.MOB + 4])};
pub const PASSED_MG: [i32; 6] = {arr(mg[F.PASSED:F.PASSED + 6])};
pub const PASSED_EG: [i32; 6] = {arr(eg[F.PASSED:F.PASSED + 6])};
pub const DOUBLED: (i32, i32) = ({mg[F.DOUBLED]}, {eg[F.DOUBLED]});
pub const ISOLATED: (i32, i32) = ({mg[F.ISOLATED]}, {eg[F.ISOLATED]});
pub const SHIELD1: (i32, i32) = ({mg[F.SHIELD1]}, {eg[F.SHIELD1]});
pub const SHIELD2: (i32, i32) = ({mg[F.SHIELD2]}, {eg[F.SHIELD2]});
pub const KING_OPEN_FILE: (i32, i32) = ({mg[F.KOPEN]}, {eg[F.KOPEN]});
pub const KING_ATTACK: (i32, i32) = ({mg[F.KATT]}, {eg[F.KATT]});
pub const ROOK_OPEN: (i32, i32) = ({mg[F.ROOK_OPEN]}, {eg[F.ROOK_OPEN]});
pub const ROOK_HALF_OPEN: (i32, i32) = ({mg[F.ROOK_HALF]}, {eg[F.ROOK_HALF]});
pub const BISHOP_PAIR: (i32, i32) = ({mg[F.BPAIR]}, {eg[F.BPAIR]});
pub const PAWN_THREAT: (i32, i32) = ({mg[F.PTHREAT]}, {eg[F.PTHREAT]});
pub const OUTPOST: (i32, i32) = ({mg[F.OUTPOST]}, {eg[F.OUTPOST]});
pub const SUPPORTED: (i32, i32) = ({mg[F.SUPPORTED]}, {eg[F.SUPPORTED]});
pub const ROOK_SEVENTH: (i32, i32) = ({mg[F.ROOK7]}, {eg[F.ROOK7]});
pub const TEMPO: (i32, i32) = ({mg[F.TEMPO]}, {eg[F.TEMPO]});
pub const CASTLE_RIGHTS: (i32, i32) = ({mg[F.CASTLE]}, {eg[F.CASTLE]});
pub const UNDEVELOPED: (i32, i32) = ({mg[F.UNDEV]}, {eg[F.UNDEV]});
"""
    path.write_text(s)


if __name__ == "__main__":
    main()
