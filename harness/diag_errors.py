"""Show HL's thought trace on positions where it loses >= 60 cp and the human did not.
usage: diag_errors.py humans.pgn hl_bin N ply_lo ply_hi [movetime_ms]"""
import sys, random, subprocess, chess, chess.pgn, chess.engine
pgn, hl, n, lo, hi = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5])
ms = int(sys.argv[6]) if len(sys.argv) > 6 else 600
rng = random.Random(5); pos = []
f = open(pgn)
while (g := chess.pgn.read_game(f)):
    b = g.board(); tc = g.headers["TimeControl"]
    elo = {True: min(3200, int(g.headers["WhiteElo"])), False: min(3200, int(g.headers["BlackElo"]))}
    ms_ = list(g.mainline_moves())
    for i, m in enumerate(ms_):
        if lo <= i < hi and rng.random() < 0.15: pos.append((b.copy(), m, elo[b.turn], tc))
        b.push(m)
rng.shuffle(pos)
sf = chess.engine.SimpleEngine.popen_uci("../dl/sf/src/stockfish"); L = chess.engine.Limit(nodes=50000)
def sc(b, m):
    b2 = b.copy(); b2.push(m); return -b2.is_checkmate() * 0 + (sf.analyse(b2, L)["score"].pov(b.turn).score(mate_score=3000))
shown = 0
for k, (b, hm, elo, tc) in enumerate(pos):
    if shown >= n: break
    base = int(tc.split("+")[0]) * 600
    out = subprocess.run([hl], input=f"uci\nsetoption name UCI_Elo value {elo}\nsetoption name Seed value {k}\nsetoption name SimTimeControl value {tc}\nposition fen {b.fen()}\ngo movetime {ms} wtime {base} btime {base}\nquit\n", capture_output=True, text=True).stdout
    mv = chess.Move.from_uci(out.split("bestmove ")[1].split()[0])
    info = sf.analyse(b, L); best = info["pv"][0]; bs = info["score"].pov(b.turn).score(mate_score=3000)
    hc, uc = bs - sc(b, mv), bs - sc(b, hm)
    if hc >= 60 and uc < 30:
        shown += 1
        print(f"=== {b.fen()} elo {elo}  SF best {b.san(best)} {bs}  human {b.san(hm)} -{uc}  HL {b.san(mv)} -{hc}")
        print("\n".join(l[12:] for l in out.splitlines() if l.startswith("info string")))
sf.quit()
