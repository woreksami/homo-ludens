import sys,json,chess.pgn,statistics
d=sys.argv[1]; an={}
for l in open(d+'/analysis_50000.jsonl'):
    r=json.loads(l); an[r['fen']]=r
for w in ('humans','homo_ludens'):
    f=open(f'{d}/{w}.pgn'); br=[]; never=0; after=[]; L=[]
    while (g:=chess.pgn.read_game(f)):
        b=g.board(); first=None; n=0
        for ply,m in enumerate(g.mainline_moves()):
            r=an.get(b.fen())
            if r and r['lines'] and first is None and abs(r['lines'][0][1])>=200: first=ply
            b.push(m); n=ply+1
        L.append(n)
        if first is None: never+=1
        else: br.append(first); after.append(n-first)
    print(w,'games',len(L),'never +-2:',never,'median break ply',statistics.median(br),'median plies after break',statistics.median(after),'mean len',round(statistics.mean(L),1))
