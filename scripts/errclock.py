import json, math, re, sys, chess.pgn, collections
D=sys.argv[1]
def wp(cp): cp=max(-1000,min(1000,cp)); return 50+50*(2/(1+math.exp(-0.00368208*cp))-1)
an={json.loads(l)["fen"]:json.loads(l) for l in open(f"{D}/analysis_10000.jsonl")}
def ev(b):
    r=an.get(b.fen()); return r["lines"][0][1] if r and not r.get("over") and r["lines"] else None
st=collections.defaultdict(collections.Counter)
f=open(f"{D}/homo_ludens.pgn")
while (g:=chess.pgn.read_game(f)):
    base,inc=map(int,g.headers["TimeControl"].split("+")); clk=[base-4,base-4]
    b=g.board(); node=g
    for i,m in enumerate(g.mainline_moves()):
        node=node.variation(0); side=int(b.turn)
        e=re.search(r"%emt ([\d.]+)",node.comment or "")
        rem=clk[side]
        best=ev(b); nb=b.copy(); nb.push(m); a=ev(nb)
        if i>=8 and best is not None and a is not None:
            drop=wp(best)-wp(-a); k="<10s" if rem<10 else "<30s" if rem<30 else "<60s" if rem<60 else "60s+"
            st[k]["n"]+=1; st[k]["inacc"]+=5<=drop<10; st[k]["mist"]+=10<=drop<15; st[k]["blun"]+=drop>=15
        if e: clk[side]-=float(e.group(1)); clk[side]+=inc
        b.push(m)
for k,v in sorted(st.items()): print(k, v["n"], {x:round(v[x]/v["n"],3) for x in ["inacc","mist","blun"]})
