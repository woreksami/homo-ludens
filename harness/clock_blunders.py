"""Blunder rate against clock left: HL's games vs the Lichess curves from
ralegionc/blunder-thermodynamics (zero-increment games, blunder = cpl >= 200,
evals clipped at 10 pawns).

usage: clock_blunders.py <hl.pgn> <analysis.jsonl> <bt_out_dir>
"""
import sys, json, csv, glob, collections, re
import chess, chess.pgn

pgn_path, cache_path, bt_dir = sys.argv[1:4]
ev = {}
for l in open(cache_path):
    d = json.loads(l)
    if d.get("lines"):
        ev[d["fen"]] = max(-1000, min(1000, d["lines"][0][1]))

BANDS = [(1000, 1299), (1300, 1599), (1600, 1899), (1900, 2199), (2200, 2599)]
BINS = [0, 2, 4, 8, 16, 32, 64, 128, 1e9]
TCS = [60, 180, 300, 600]

def band(e):
    for i, (lo, hi) in enumerate(BANDS):
        if lo <= e <= hi:
            return i
    return None

def cbin(t):
    for i in range(len(BINS) - 1):
        if BINS[i] <= t < BINS[i + 1]:
            return i

acc = collections.defaultdict(lambda: [0, 0])
f = open(pgn_path)
while True:
    g = chess.pgn.read_game(f)
    if g is None:
        break
    tc = g.headers.get("TimeControl", "-")
    if "+" not in tc:
        continue
    base, inc = map(int, tc.split("+"))
    if inc != 0:
        continue
    tcn = min(TCS, key=lambda x: abs(x - base))
    elos = [int(g.headers.get("WhiteElo", 0)), int(g.headers.get("BlackElo", 0))]
    clock = [float(base), float(base)]
    b = g.board()
    for node in g.mainline():
        side = 0 if b.turn == chess.WHITE else 1
        before = clock[side]
        m = re.search(r'%emt ([0-9.]+)', node.comment)
        emt = float(m.group(1)) if m else None
        fen = b.fen()
        b.push(node.move)
        if emt is not None:
            clock[side] -= emt
        bi = band(elos[side])
        if emt is None or bi is None or fen not in ev or b.fen() not in ev or b.is_game_over():
            continue
        cpl = max(0, ev[fen] + ev[b.fen()])
        k = (tcn, bi, cbin(before))
        acc[k][0] += cpl >= 200
        acc[k][1] += 1

ref = {}
for p in glob.glob(f"{bt_dir}/2024-06_*p0_curves.csv"):
    if "_fine" in p or "_int" in p or "_t" in p.split("p0")[1]:
        continue
    for r in csv.DictReader(open(p)):
        tcn = int(r["tc"].split("+")[0])
        k = (tcn, int(r["band_i"]), cbin(float(r["clk"])))
        a = ref.setdefault(k, [0.0, 0])
        a[0] += float(r["rate_raw"]) * int(r["n_moves"])
        a[1] += int(r["n_moves"])

print("tc   band        clock      HL rate (n)     Lichess rate")
tot = collections.defaultdict(lambda: [0, 0, 0.0])
for k in sorted(acc):
    hb, hn = acc[k]
    if hn < 15:
        continue
    r = ref.get(k)
    rr = r[0] / r[1] if r else float("nan")
    lo, hi = BINS[k[2]], BINS[k[2] + 1]
    print(f"{k[0]:<4} {BANDS[k[1]][0]}-{BANDS[k[1]][1]}  {lo:>4}-{hi if hi < 1e9 else 'inf':<5} {hb / hn:6.3f} ({hn:4d})   {rr:6.3f}")
    low = "low (<16s)" if hi <= 16 else "high"
    t = tot[low]
    t[0] += hb; t[1] += hn; t[2] += rr * hn
for k, t in tot.items():
    print(f"{k}: HL {t[0] / t[1]:.3f}  Lichess (same mix) {t[2] / t[1]:.3f}  n={t[1]}")
