import sys, json, chess.pgn, statistics
an = {}
for line in open(sys.argv[1] + "/analysis_50000.jsonl"):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
def ev(b):
    r = an.get(" ".join(b.fen().split()[:4]))
    if not r or not r["lines"]: return None
    return r["lines"][0][1]  # side to move pov
for who in ("humans", "homo_ludens"):
    f = open(f"{sys.argv[1]}/{who}.pgn"); W=[];L=[];Wt=[];Lt=[]
    while (g := chess.pgn.read_game(f)):
        b = g.board()
        for m in g.mainline_moves():
            v = ev(b)
            if v is not None and 300 <= abs(v) <= 1500:
                b2=b.copy(); b2.push(m); v2=ev(b2)
                if v2 is not None:
                    loss=max(0,min(v,1500)-min(-v2,1500))
                    (W if v>0 else L).append(min(loss,1000))
            b.push(m)
    print(who, "winning-side ACPL %.0f n%d | losing-side ACPL %.0f n%d"%(statistics.mean(W),len(W),statistics.mean(L),len(L)), " ".join("%d:%.3f/%.3f"%(t,sum(x>t for x in W)/len(W),sum(x>t for x in L)/len(L)) for t in (50,100,300,500,800)), "zero %.2f/%.2f"%(sum(x<10 for x in W)/len(W),sum(x<10 for x in L)/len(L)))
