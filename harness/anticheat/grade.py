"""Grade bots against real human games.

1. lichess's move-quality game flags
2. regan-style stats per rating band, bot vs human, with z-scores
3. detectors (logistic + boosted trees) that try to tell bot player-games
   from human ones at matched ratings/openings. AUC 0.50 = can't tell.

The detector is only a ruler for how human the play is, see README.

  python harness/anticheat/grade.py --dir reports/anticheat --bots homo_ludens maia engine
"""

from __future__ import annotations

import argparse
import json
import math
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

import chess
import chess.pgn
import numpy as np

OPENING_PLIES = 8
EMT = re.compile(r"\[%emt ([\d.]+)\]")
FEATURES = [
    "acpl", "accuracy", "top1_match", "regan_error", "inaccuracy_rate", "mistake_rate", "blunder_rate",
    "match_critical", "match_ambiguous", "acpl_when_worse", "never_worse", "acpl_opening", "acpl_middlegame",
    "acpl_endgame", "game_plies",
]
# Used only when the human games carry move times too.
TIME_FEATURES = ["emt_cv", "emt_fast_share", "emt_rel", "emt_long_share"]
LABELS = {
    "acpl": "Average centipawn loss",
    "accuracy": "Lichess accuracy %",
    "top1_match": "Engine top-move match",
    "regan_error": "Regan scaled error",
    "inaccuracy_rate": "Inaccuracy rate",
    "mistake_rate": "Mistake rate",
    "blunder_rate": "Blunder rate",
    "match_critical": "Top-move match, only-move positions",
    "match_ambiguous": "Top-move match, several good moves",
    "acpl_when_worse": "ACPL when worse by 1+ pawn",
    "never_worse": "Never worse than -1 (Lichess flag)",
    "acpl_opening": "ACPL, moves 5-10",
    "acpl_middlegame": "ACPL, middlegame",
    "acpl_endgame": "ACPL, endgame",
    "game_plies": "Game length (plies)",
    "emt_cv": "Move-time CV",
    "emt_fast_share": "Share of moves under 0.5 s",
    "emt_rel": "Mean move time / nominal (base/40 + inc)",
    "emt_long_share": "Share of moves over 3x nominal",
}


def nominal_secs(tc: str) -> float:
    try:
        base, inc = (float(x) for x in tc.split("+"))
    except ValueError:
        return float("nan")
    return base / 40 + inc


def win_pct(cp: float) -> float:
    return 50 + 50 * (2 / (1 + math.exp(-0.00368208 * cp)) - 1)


def move_accuracy(before_wp: float, after_wp: float) -> float:
    if after_wp >= before_wp:
        return 100.0
    raw = 103.1668100711649 * math.exp(-0.04354415386753951 * (before_wp - after_wp)) - 3.166924740191411 + 1
    return max(0.0, min(100.0, raw))


def lichess_game_accuracy(seq: list[tuple[bool, float, float, float]]) -> dict[bool, float]:
    """Lichess's game accuracy (lila AccuracyPercent.gameAccuracy), per side.

    seq: per analysed move, (side, white win% before, white win% after, move accuracy).
    Each move is weighted by the volatility (std dev of win%) of a sliding window
    around it; the game accuracy is the mean of that weighted mean and the
    harmonic mean of the move accuracies, so a single blunder costs a lot.
    """
    if not seq:
        return {}
    wps = [seq[0][1]] + [x[2] for x in seq]
    n = len(seq)
    w = max(2, min(8, n // 10))
    windows = [wps[:w]] * max(0, min(w, len(wps)) - 2) + [wps[i:i + w] for i in range(len(wps) - w + 1)]
    weights = [max(0.5, min(12.0, statistics.pstdev(win))) for win in windows[:n]]
    out = {}
    for side in (chess.WHITE, chess.BLACK):
        pairs = [(x[3], weights[i]) for i, x in enumerate(seq) if x[0] == side and i < len(weights)]
        if not pairs:
            continue
        weighted = sum(a * k for a, k in pairs) / sum(k for _, k in pairs)
        harmonic = len(pairs) / sum(1 / max(a, 1.0) for a, _ in pairs)
        out[side] = (weighted + harmonic) / 2
    return out


def regan_F(cp: float) -> float:
    v = cp / 100
    return math.copysign(math.log1p(abs(v)), v)


def speed_of(tc: str) -> str:
    try:
        base, inc = (int(x) for x in tc.split("+"))
    except ValueError:
        return "classical"
    est = base + 40 * inc
    return "bullet" if est < 180 else "blitz" if est < 480 else "rapid" if est < 1500 else "classical"


def non_pawn_material(b: chess.Board) -> int:
    vals = {chess.KNIGHT: 3, chess.BISHOP: 3, chess.ROOK: 5, chess.QUEEN: 9}
    return sum(len(b.pieces(p, c)) * v for p, v in vals.items() for c in (chess.WHITE, chess.BLACK))


def load_analysis(path: Path) -> dict:
    out = {}
    for line in open(path):
        r = json.loads(line)
        out[r["fen"]] = r
    return out


def after_eval_for_mover(an: dict, b_after: chess.Board) -> float | None:
    """Eval of the position after the move, from the mover's point of view."""
    if b_after.is_checkmate():
        return 2000.0
    if b_after.is_game_over():
        return 0.0
    r = an.get(b_after.fen())
    if not r or not r["lines"]:
        return None
    return -r["lines"][0][1]


def player_games(pgn: Path, an: dict, corpus: str) -> list[dict]:
    rows = []
    with open(pgn, encoding="utf-8", errors="replace") as f:
        idx = -1
        while (g := chess.pgn.read_game(f)) is not None:
            idx += 1
            h = g.headers
            game_id = h.get("Site", "").rsplit("/", 1)[-1]
            if not any(ch.isdigit() for ch in game_id):
                game_id = f"g{idx}"  # sites without ids (Chess.com sets): pair by file order
            per = {chess.WHITE: defaultdict(list), chess.BLACK: defaultdict(list)}
            seq = []
            b = g.board()
            node = g
            for ply, m in enumerate(g.mainline_moves()):
                node = node.variation(0)
                side = b.turn
                if ply >= OPENING_PLIES:
                    r = an.get(b.fen())
                    nb = b.copy()
                    nb.push(m)
                    after = after_eval_for_mover(an, nb)
                    if r and r["lines"] and after is not None:
                        lines = r["lines"]
                        best = lines[0][1]
                        cap = lambda x: max(-1000, min(1000, x))  # noqa: E731
                        loss = max(0, cap(best) - cap(after))
                        wb, wa = win_pct(cap(best)), win_pct(cap(after))
                        d = per[side]
                        d["loss"].append(loss)
                        d["acc"].append(move_accuracy(wb, wa))
                        white_view = (lambda x: x) if side == chess.WHITE else (lambda x: 100 - x)  # noqa: E731
                        seq.append((side, white_view(wb), white_view(wa), d["acc"][-1]))
                        drop = wb - wa
                        d["inacc"].append(1 if 5 <= drop < 10 else 0)
                        d["mistake"].append(1 if 10 <= drop < 15 else 0)
                        d["blunder"].append(1 if drop >= 15 else 0)
                        d["evals"].append(best)
                        top1 = int(m.uci() == lines[0][0])
                        good = sum(1 for _, cp in lines if win_pct(cap(best)) - win_pct(cap(cp)) < 5)
                        second_gap = wb - win_pct(cap(lines[1][1])) if len(lines) > 1 else 100
                        if second_gap >= 10:
                            d["m_crit"].append(top1)
                        elif good >= 2:
                            d["m_amb"].append(top1)
                        if abs(best) <= 300:
                            d["top1"].append(top1)
                            d["regan"].append(max(0.0, regan_F(best) - regan_F(after)))
                        if best < -100:
                            d["loss_worse"].append(loss)
                        if ply < 20:
                            d["loss_open"].append(loss)
                        elif non_pawn_material(b) <= 26:
                            d["loss_end"].append(loss)
                        else:
                            d["loss_mid"].append(loss)
                        mt = EMT.search(node.comment or "")
                        if mt:
                            d["emt"].append(float(mt.group(1)))
                b.push(m)
            game_acc = lichess_game_accuracy(seq)
            for side in (chess.WHITE, chess.BLACK):
                d = per[side]
                if len(d["loss"]) < 10:
                    continue
                mean = lambda k: statistics.fmean(d[k]) if d[k] else float("nan")  # noqa: E731
                elo = int(h["WhiteElo" if side else "BlackElo"])
                emt = d["emt"]
                rows.append({
                    "corpus": corpus,
                    "game": game_id,
                    "side": "white" if side else "black",
                    "elo": elo,
                    "speed": speed_of(h.get("TimeControl", "-")),
                    "moves": len(d["loss"]),
                    "acpl": mean("loss"),
                    "accuracy": game_acc.get(side, float("nan")),
                    "move_accuracy": mean("acc"),
                    "top1_match": mean("top1"),
                    "regan_error": mean("regan"),
                    "inaccuracy_rate": mean("inacc"),
                    "mistake_rate": mean("mistake"),
                    "blunder_rate": mean("blunder"),
                    "match_critical": mean("m_crit"),
                    "match_ambiguous": mean("m_amb"),
                    "acpl_when_worse": mean("loss_worse"),
                    "never_worse": float(all(e >= -100 for e in d["evals"])),
                    "acpl_opening": mean("loss_open"),
                    "acpl_middlegame": mean("loss_mid"),
                    "acpl_endgame": mean("loss_end"),
                    "game_plies": float(g.end().ply()),
                    "emt_cv": (statistics.pstdev(emt) / statistics.fmean(emt)) if len(emt) >= 10 and statistics.fmean(emt) > 0 else float("nan"),
                    "emt_fast_share": (sum(t < 0.5 for t in emt) / len(emt)) if len(emt) >= 10 else float("nan"),
                    "emt_rel": (statistics.fmean(emt) / nominal_secs(h.get("TimeControl", "-"))) if len(emt) >= 10 else float("nan"),
                    "emt_long_share": (sum(t > 3 * nominal_secs(h.get("TimeControl", "-")) for t in emt) / len(emt)) if len(emt) >= 10 else float("nan"),
                })
    return rows


def lichess_flags(r: dict) -> dict:
    limit = {"bullet": 25, "blitz": 20}.get(r["speed"], 15)
    return {"suspicious_error_rate": r["acpl"] < limit, "always_has_advantage": r["never_worse"] == 1.0}


def z_two_sample(a: list[float], b: list[float]) -> float:
    a = [x for x in a if not math.isnan(x)]
    b = [x for x in b if not math.isnan(x)]
    if len(a) < 3 or len(b) < 3:
        return float("nan")
    se = math.sqrt(statistics.variance(a) / len(a) + statistics.variance(b) / len(b))
    return (statistics.fmean(a) - statistics.fmean(b)) / se if se > 0 else float("nan")


def detector(humans: list[dict], bots: list[dict], features: list[str], seed: int = 0) -> dict:
    from sklearn.ensemble import GradientBoostingClassifier
    from sklearn.impute import SimpleImputer
    from sklearn.linear_model import LogisticRegression
    from sklearn.metrics import roc_auc_score
    from sklearn.model_selection import GroupKFold
    from sklearn.pipeline import make_pipeline
    from sklearn.preprocessing import StandardScaler

    rows = humans + bots
    X = np.array([[r[f] for f in features] + [r["elo"]] for r in rows], dtype=float)
    y = np.array([0] * len(humans) + [1] * len(bots))
    # Keep a human game and the bot replay of that same game in the same fold.
    groups = np.array([r["game"] for r in rows])
    out = {}
    models = {
        "logistic": lambda: make_pipeline(SimpleImputer(), StandardScaler(), LogisticRegression(max_iter=2000)),
        "boosted_trees": lambda: make_pipeline(SimpleImputer(), GradientBoostingClassifier(random_state=seed)),
    }
    for name, make in models.items():
        scores = np.zeros(len(y))
        for tr, te in GroupKFold(n_splits=5).split(X, y, groups):
            m = make().fit(X[tr], y[tr])
            scores[te] = m.predict_proba(X[te])[:, 1]
        out[name] = roc_auc_score(y, scores)
    # Which single signals give the bot away? AUC of each feature alone (0.5 = no signal).
    single = {}
    for i, f in enumerate(features):
        col = X[:, i]
        ok = ~np.isnan(col)
        if ok.sum() > 10 and len(set(y[ok])) == 2:
            auc = roc_auc_score(y[ok], col[ok])
            single[f] = max(auc, 1 - auc)
    out["single_feature_auc"] = dict(sorted(single.items(), key=lambda kv: -kv[1]))
    return out


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dir", type=Path, default=Path("reports/anticheat"))
    ap.add_argument("--bots", nargs="+", default=["homo_ludens", "maia", "engine"])
    ap.add_argument("--nodes", type=int, default=10_000, help="which analysis cache to grade with (0 = legacy analysis.jsonl, 30k)")
    ap.add_argument("--band-width", type=int, default=200)
    ap.add_argument("--out", type=Path)
    args = ap.parse_args()
    an = load_analysis(args.dir / (f"analysis_{args.nodes}.jsonl" if args.nodes else "analysis.jsonl"))

    corpora = {"humans": player_games(args.dir / "humans.pgn", an, "humans")}
    for bot in args.bots:
        p = args.dir / f"{bot}.pgn"
        if p.exists() and p.stat().st_size > 0:
            corpora[bot] = player_games(p, an, bot)
    humans = corpora["humans"]
    report = {"player_games": {k: len(v) for k, v in corpora.items()}, "bots": {}}

    for bot, rows in corpora.items():
        if bot == "humans":
            continue
        # Restrict humans to the games this bot replayed (the engine control uses a subset).
        replayed = {r["game"] for r in rows}
        hs = [r for r in humans if r["game"] in replayed]
        rep = {}
        # 1. Lichess flags.
        rep["lichess_flags"] = {
            who: {k: statistics.fmean(lichess_flags(r)[k] for r in rs) for k in ("suspicious_error_rate", "always_has_advantage")}
            for who, rs in (("humans", hs), (bot, rows))
        }
        cv = [r["emt_cv"] for r in rows if not math.isnan(r["emt_cv"])]
        if cv:
            fast = [r["emt_fast_share"] for r in rows]
            n_moves = [r["moves"] for r in rows]
            rep["time_flags_bot_only"] = {
                "median_move_time_cv": statistics.median(cv),
                "share_highly_consistent_cv_below_0.25": statistics.fmean(c < 0.25 for c in cv),
                "share_moderately_consistent_cv_below_0.40": statistics.fmean(c < 0.40 for c in cv),
                "share_no_fast_moves_flag": statistics.fmean(f * n <= n / 20 + 2 for f, n in zip(fast, n_moves)),
            }
        # 2. Means and Z-scores, overall and per band.
        rep["metrics"] = {}
        for f in FEATURES:
            hv = [r[f] for r in hs if not math.isnan(r[f])]
            bv = [r[f] for r in rows if not math.isnan(r[f])]
            rep["metrics"][f] = {
                "human_mean": statistics.fmean(hv) if hv else None,
                "bot_mean": statistics.fmean(bv) if bv else None,
                "z": z_two_sample(bv, hv),
            }
        bands = defaultdict(lambda: ([], []))
        for r in hs:
            bands[(r["elo"] // args.band_width) * args.band_width][0].append(r)
        for r in rows:
            bands[(r["elo"] // args.band_width) * args.band_width][1].append(r)
        rep["bands"] = {
            str(b): {f: {"human": statistics.fmean([r[f] for r in h if not math.isnan(r[f])] or [float("nan")]),
                         "bot": statistics.fmean([r[f] for r in bt if not math.isnan(r[f])] or [float("nan")]),
                         "z": z_two_sample([r[f] for r in bt], [r[f] for r in h])}
                     for f in ("acpl", "accuracy", "top1_match", "blunder_rate")}
            for b, (h, bt) in sorted(bands.items())
        }
        # 3. Detector, with and without game length (length is partly a matter of
        # how the replay ends, so it is reported both ways).
        rep["detector"] = detector(hs, rows, FEATURES)
        rep["detector_without_length"] = {k: v for k, v in detector(hs, rows, [f for f in FEATURES if f != "game_plies"]).items() if k != "single_feature_auc"}
        human_timed = sum(not math.isnan(r["emt_cv"]) for r in hs) > 0.5 * max(1, len(hs))
        if human_timed:
            rep["detector_with_time"] = detector(hs, rows, [f for f in FEATURES if f != "game_plies"] + TIME_FEATURES)
            rep["time_metrics"] = {f: {"human": statistics.fmean([r[f] for r in hs if not math.isnan(r[f])] or [float("nan")]),
                                       "bot": statistics.fmean([r[f] for r in rows if not math.isnan(r[f])] or [float("nan")]),
                                       "z": z_two_sample([r[f] for r in rows], [r[f] for r in hs])} for f in TIME_FEATURES}
        report["bots"][bot] = rep

    out = args.out or (args.dir / "grade.json")
    out.write_text(json.dumps(report, indent=2, default=float))

    # Human-readable summary.
    lines = ["# Anti-cheat grade\n", f"Player-games: {report['player_games']}\n"]
    lines.append("## Detector (AUC, 0.50 = indistinguishable from humans)\n")
    lines.append("| Bot | Logistic | Boosted trees | Boosted, without game length |")
    lines.append("| --- | ---: | ---: | ---: |")
    for bot, rep in report["bots"].items():
        d, d2 = rep["detector"], rep["detector_without_length"]
        lines.append(f"| {bot} | {d['logistic']:.2f} | {d['boosted_trees']:.2f} | {d2['boosted_trees']:.2f} |")
    for bot, rep in report["bots"].items():
        lines.append(f"\n## {bot}\n")
        fl = rep["lichess_flags"]
        lines.append(f"Lichess flags, humans vs {bot}: suspicious error rate {fl['humans']['suspicious_error_rate']:.0%} vs "
                     f"{fl[bot]['suspicious_error_rate']:.0%}; always has advantage {fl['humans']['always_has_advantage']:.0%} vs "
                     f"{fl[bot]['always_has_advantage']:.0%}\n")
        if "time_flags_bot_only" in rep:
            t = rep["time_flags_bot_only"]
            lines.append(f"Time flags (bot only; no human clocks in this data): median move-time CV {t['median_move_time_cv']:.2f}; "
                         f"'highly consistent' {t['share_highly_consistent_cv_below_0.25']:.0%}; "
                         f"'no fast moves' {t['share_no_fast_moves_flag']:.0%} of games\n")
        lines.append("| Signal | Humans | Bot | Z |")
        lines.append("| --- | ---: | ---: | ---: |")
        for f in FEATURES:
            m = rep["metrics"][f]
            if m["human_mean"] is None:
                continue
            lines.append(f"| {LABELS[f]} | {m['human_mean']:.3g} | {m['bot_mean']:.3g} | {m['z']:+.1f} |")
        if "detector_with_time" in rep:
            dt = rep["detector_with_time"]
            lines.append(f"\nWith move times (no game length): logistic {dt['logistic']:.2f}, boosted {dt['boosted_trees']:.2f}\n")
            for f, m in rep["time_metrics"].items():
                lines.append(f"| {LABELS[f]} | {m['human']:.3g} | {m['bot']:.3g} | {m['z']:+.1f} |")
            tt = [(k, v) for k, v in dt["single_feature_auc"].items() if k in TIME_FEATURES]
            lines.append("\nTime giveaways (single-signal AUC): " + ", ".join(f"{LABELS[k]} {v:.2f}" for k, v in tt) + "\n")
        top = list(rep["detector"]["single_feature_auc"].items())[:5]
        lines.append("\nStrongest giveaways (single-signal AUC): " + ", ".join(f"{LABELS[k]} {v:.2f}" for k, v in top))
    md = "\n".join(lines) + "\n"
    out.with_suffix(".md").write_text(md)
    print(md)


if __name__ == "__main__":
    main()
