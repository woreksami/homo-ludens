import json, math, sys, chess.pgn, collections
D=sys.argv[1]
def wp(cp): cp=max(-1000,min(1000,cp)); return 50+50*(2/(1+math.exp(-0.00368208*cp))-1)
an={}
for l in open(f"{D}/analysis_10000.jsonl"):
    r=json.loads(l); an[r["fen"]]=r
def ev(b):
    r=an.get(b.fen())
    if not r: return None
    if r.get("over"): return None
    return r["lines"][0][1] if r["lines"] else None
for name in ["humans","homo_ludens"]:
    st=collections.defaultdict(lambda: collections.Counter())
    f=open(f"{D}/{name}.pgn")
    while (g:=chess.pgn.read_game(f)):
        b=g.board()
        for i,m in enumerate(g.mainline_moves()):
            best=ev(b); nb=b.copy(); nb.push(m); a=ev(nb)
            if i>=8 and best is not None and a is not None:
                drop=wp(best)-wp(-a)
                k="bal" if abs(best)<100 else "edge" if abs(best)<300 else "decided"
                st[k]["n"]+=1; st[k]["inacc"]+= 5<=drop<10; st[k]["mist"]+=10<=drop<15; st[k]["blun"]+=drop>=15; st[k]["zero"]+=drop<1
            b.push(m)
    print(name, {k:{x:round(v[x]/v['n'],3) for x in ['inacc','mist','blun','zero']}|{'n':v['n']} for k,v in st.items()})
