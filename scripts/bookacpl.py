import sys, json, math, chess, chess.pgn, collections, statistics
exec(open(sys.argv[3]).read().split("f = open")[0])  # reuse book + total()
an = {}
for line in open(sys.argv[2]):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
def val(fen):
    r = an.get(" ".join(fen.split()[:4]))
    if not r or not r["lines"]: return None
    return r["lines"][0][1]
f = open(sys.argv[1]); res = collections.defaultdict(list)
while (g := chess.pgn.read_game(f)):
    b = g.board()
    for i, m in enumerate(g.mainline_moves()):
        mv = i//2+1
        if 5 <= mv <= 10:
            elo = int(g.headers["WhiteElo" if b.turn else "BlackElo"])
            inb = total(b, elo) >= 1.5
            v0 = val(b.fen()); b2 = b.copy(); b2.push(m); v1 = val(b2.fen())
            if v0 is not None and v1 is not None:
                res[inb].append(max(0, min(1000, v0) - min(1000, -v1)))
        b.push(m)
for k, v in res.items(): print("inbook" if k else "out   ", len(v), round(statistics.fmean(v),1), "top-ish(<=10)", round(statistics.fmean(x<=10 for x in v),2))
