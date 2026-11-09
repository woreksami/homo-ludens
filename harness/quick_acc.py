"""Quick accuracy probe: HL's move vs the human's move on the same positions.
usage: quick_acc.py humans.pgn hl_binary N [ENV=VAL ...]
Prints ACPL, top-move match and inaccuracy+ rate for humans and HL, by phase."""
import sys, os, random, statistics, chess, chess.pgn, chess.engine

pgn, hl_bin, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
env = dict(os.environ, **dict(a.split("=", 1) for a in sys.argv[4:]))
SF = "../dl/sf/src/stockfish"
rng = random.Random(1)
pos = []
f = open(pgn)
while (g := chess.pgn.read_game(f)):
    b = g.board(); moves = list(g.mainline_moves()); tc = g.headers.get("TimeControl", "180+0")
    elo = {True: int(g.headers["WhiteElo"]), False: int(g.headers["BlackElo"])}
    for i, m in enumerate(moves):
        if int(os.environ.get('PLYMIN', 10)) <= i < min(len(moves) - 2, int(os.environ.get('PLYMAX', 80))) and rng.random() < float(os.environ.get('PSAMP', 0.08)):
            pos.append((b.copy(), m, elo[b.turn], tc, i))
        b.push(m)
rng.shuffle(pos); pos = pos[:n]
sf = chess.engine.SimpleEngine.popen_uci(SF)
hl = chess.engine.SimpleEngine.popen_uci(os.path.abspath(hl_bin), env=env, cwd=os.getcwd())
lim = chess.engine.Limit(nodes=50000)
def cpl(b, m, best_score):
    b2 = b.copy(); b2.push(m)
    if b2.is_game_over():
        return 0
    s = sf.analyse(b2, lim)["score"].pov(b.turn).score(mate_score=3000)
    return max(0, min(1000, best_score) - min(1000, s))
res = {"humans": [], "hl": []}
for k, (b, hm, elo, tc, ply) in enumerate(pos):
    info = sf.analyse(b, lim)
    best = info["pv"][0]; bs = info["score"].pov(b.turn).score(mate_score=3000)
    base = int(tc.split("+")[0])
    hl.configure({"UCI_Elo": min(3200, elo), "Seed": 7 + k, "SimTimeControl": tc})
    if os.environ.get("RAW"): hl.configure({"UCI_LimitStrength": False})
    r = hl.play(b, chess.engine.Limit(time=float(os.environ.get("QT", "0.2")), white_clock=base * 0.6, black_clock=base * 0.6))
    phase = "open" if ply < 20 else ("mid" if ply < 60 else "late")
    for who, m in (("humans", hm), ("hl", r.move)):
        res[who].append((cpl(b, m, bs), m == best, phase))
sf.quit(); hl.quit()
for who, rows in res.items():
    c = [x[0] for x in rows]
    print(f"{who:7s} ACPL {statistics.fmean(c):5.1f}  top {statistics.fmean(x[1] for x in rows):.3f}  inacc+ {statistics.fmean(x >= 50 for x in c):.3f}  exact {statistics.fmean(x < 10 for x in c):.3f}  10-100 {statistics.fmean(10 <= x < 100 for x in c):.3f}  300+ {statistics.fmean(x >= 300 for x in c):.3f}  "
          + "  ".join(f"{ph} {statistics.fmean(v):5.1f}" for ph in ("open", "mid", "late") if (v := [x[0] for x in rows if x[2] == ph])))
