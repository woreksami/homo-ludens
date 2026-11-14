import sys,json,chess.pgn,statistics
d=sys.argv[1]; an={}
for l in open(d+'/analysis_50000.jsonl'):
    r=json.loads(l); an[r['fen']]=r
for w in ('humans','homo_ludens'):
    f=open(f'{d}/{w}.pgn'); E=[]; mates=0; flags=0
    while (g:=chess.pgn.read_game(f)):
        if g.headers['Result'] not in ('1-0','0-1'): continue
        b=g.end().board()
        if b.is_checkmate(): mates+=1; continue
        if 'time' in g.headers.get('Termination','').lower(): flags+=1; continue
        r=an.get(b.fen())
        if r and r['lines']:
            v=r['lines'][0][1]; loser_white = g.headers['Result']=='0-1'
            pov = v if b.turn==chess.WHITE else -v  # white pov
            E.append(abs(min(1500,max(-1500,pov))))
    q=statistics.quantiles(E,n=4)
    print(w,'mates',mates,'flags',flags,'resigns',len(E),'resign |eval| quartiles',[round(x) for x in q])
