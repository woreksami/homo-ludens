"""Check the Rust evaluation equals the Python feature model with the weights in src/eval_params.rs."""
import sys, re, json, random, subprocess
from pathlib import Path
import numpy as np, chess
sys.path.insert(0, str(Path(__file__).resolve().parent))
import features as F
src = Path("src/eval_params.rs").read_text()
def nums(name):
    return [int(x) for x in re.findall(r"-?\d+", re.search(name + r": [^=]*= (.*?);", src, re.S).group(1))]
mg = np.zeros(F.N_TERMS); eg = np.zeros(F.N_TERMS)
mg[F.MAT:F.MAT+5] = nums("MAT_MG"); eg[F.MAT:F.MAT+5] = nums("MAT_EG")
pm = nums("PST_MG"); pe = nums("PST_EG")
mg[F.PST:F.PST+384] = pm[-384:]; eg[F.PST:F.PST+384] = pe[-384:]
mg[F.MOB:F.MOB+4] = nums("MOB_MG")[-4:]; eg[F.MOB:F.MOB+4] = nums("MOB_EG")[-4:]
mg[F.PASSED:F.PASSED+6] = nums("PASSED_MG")[-6:]; eg[F.PASSED:F.PASSED+6] = nums("PASSED_EG")[-6:]
for t, name in [(F.DOUBLED, "DOUBLED"), (F.ISOLATED, "ISOLATED"), (F.SHIELD1, "SHIELD1"), (F.SHIELD2, "SHIELD2"), (F.KOPEN, "KING_OPEN_FILE"),
                (F.KATT, "KING_ATTACK"), (F.ROOK_OPEN, "ROOK_OPEN"), (F.ROOK_HALF, "ROOK_HALF_OPEN"), (F.BPAIR, "BISHOP_PAIR"),
                (F.PTHREAT, "PAWN_THREAT"), (F.OUTPOST, "OUTPOST"), (F.SUPPORTED, "SUPPORTED"), (F.ROOK7, "ROOK_SEVENTH"), (F.TEMPO, "TEMPO"),
                (F.CASTLE, "CASTLE_RIGHTS"), (F.UNDEV, "UNDEVELOPED"), (F.MTHREAT, "MINOR_THREAT"), (F.BACKWARD, "BACKWARD")]:
    a, b = nums(name)[-2:]; mg[t], eg[t] = a, b
fens = [json.loads(l)["fen"] for l in open("reports/t_v26/analysis_50000.jsonl")]
random.Random(0).shuffle(fens); fens = fens[:300]
inp = "".join(f"position fen {f}\neval\n" for f in fens) + "quit\n"
out = [l for l in subprocess.run(["bin/" + sys.argv[1]], input=inp, capture_output=True, text=True).stdout.splitlines() if l.startswith("eval ")]
bad = 0
for f, l in zip(fens, out):
    x, ph = F.features(chess.Board(f)); v = np.zeros(F.N_TERMS)
    for k, val in x.items(): v[k] = val
    py = (v @ mg) * ph / 24 + (v @ eg) * (1 - ph / 24); rs = int(l.split()[1])
    if abs(py - rs) > 3:
        bad += 1
        if bad <= 5: print("mismatch", f, py, rs)
print(f"{len(out)} positions, {bad} mismatches")
