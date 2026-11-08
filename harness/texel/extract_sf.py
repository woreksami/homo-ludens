"""Quiet positions with Stockfish (50k-node) evals from the analysis caches, as tuning rows.
Target: win probability 1/(1+10^(-0.85*cp/400)) from White's view (K fixed at 0.85 keeps the eval in centipawns)."""
import sys, json, glob, random
from multiprocessing import Pool
from pathlib import Path
import chess, numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parent))
from features import N_TERMS, features

def work(item):
    fen, cp, best = item
    b = chess.Board(fen)
    if b.is_check() or b.fullmove_number < 5:
        return None
    m = chess.Move.from_uci(best)
    if b.is_capture(m) or m.promotion:
        return None
    if any(b.is_capture(x) and b.piece_at(x.to_square) and b.piece_at(x.to_square).piece_type >= chess.KNIGHT for x in b.legal_moves):
        return None
    x, ph = features(b)
    w = cp if b.turn == chess.WHITE else -cp
    return x, ph, 1 / (1 + 10 ** (-0.85 * w / 400))

if __name__ == "__main__":
    seen = {}
    for p in glob.glob("reports/*/analysis_50000.jsonl"):
        for l in open(p):
            try: d = json.loads(l)
            except Exception: continue
            if d.get("lines") and abs(d["lines"][0][1]) < 1500:
                seen[d["fen"]] = (d["lines"][0][1], d["lines"][0][0])
    items = [(f, cp, bm) for f, (cp, bm) in seen.items()]
    random.Random(0).shuffle(items)
    rows = []
    with Pool(2) as pool:
        for r in pool.imap_unordered(work, items, chunksize=500):
            if r: rows.append(r)
    X = np.zeros((len(rows), N_TERMS), dtype=np.float32); ph = np.zeros(len(rows), dtype=np.float32); y = np.zeros(len(rows), dtype=np.float32)
    for i, (x, p, t) in enumerate(rows):
        for k, v in x.items(): X[i, k] = v
        ph[i] = p; y[i] = t
    np.savez_compressed("reports/texel/data_sf.npz", X=X, ph=ph, y=y)
    print(len(items), "positions,", len(rows), "quiet rows")
