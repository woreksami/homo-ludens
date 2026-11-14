import sys, json, chess, chess.pgn, collections
d=sys.argv[1]; an={}
for l in open(d+'/analysis_50000.jsonl'):
    r=json.loads(l); an[r['fen']]=r
def ev(b):
    r=an.get(b.fen())
    if not r or not r['lines']: return None
    return r['lines'][0][1]
cap=lambda x:max(-1000,min(1000,x))
for w in ('humans','homo_ludens'):
    byply=collections.defaultdict(lambda:[0,0]); byev=collections.defaultdict(lambda:[0,0]); byclk=collections.defaultdict(lambda:[0,0])
    f=open(f'{d}/{w}.pgn')
    while (g:=chess.pgn.read_game(f)):
        b=g.board(); node=g
        for ply,m in enumerate(g.mainline_moves()):
            node=node.variation(0)
            e0=ev(b); nb=b.copy(); nb.push(m); e1=ev(nb)
            if ply>=10 and e0 is not None and e1 is not None:
                loss=max(0,cap(e0)-cap(-e1)); bl=loss>=300
                k=min(ply//20,6); byply[k][0]+=bl; byply[k][1]+=1
                k2=min(abs(cap(e0))//150,4); byev[k2][0]+=bl; byev[k2][1]+=1
                c=node.clock()
                if c is not None:
                    k3=0 if c<15 else 1 if c<45 else 2 if c<90 else 3
                    byclk[k3][0]+=bl; byclk[k3][1]+=1
            b=nb
    print(w)
    print(' ply/20 ', ' '.join(f'{k}:{v[0]/v[1]*100:.1f}%({v[1]})' for k,v in sorted(byply.items())))
    print(' |ev|/150', ' '.join(f'{k}:{v[0]/v[1]*100:.1f}%({v[1]})' for k,v in sorted(byev.items())))
    print(' clk<15,45,90,+', ' '.join(f'{k}:{v[0]/v[1]*100:.1f}%({v[1]})' for k,v in sorted(byclk.items())))
