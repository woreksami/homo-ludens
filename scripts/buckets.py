import sys,json,chess.pgn,statistics
an={}
for line in open(sys.argv[1]+'/analysis_50000.jsonl'):
    r=json.loads(line); an[" ".join(r["fen"].split()[:4])]=r
for w in ('humans','homo_ludens'):
    f=open(f'{sys.argv[1]}/{w}.pgn'); B={k:[] for k in ('lt300','300-500','500-900','900-1500','1500+')}; back=[]
    while (g:=chess.pgn.read_game(f)):
        if g.headers['Result'] not in ('1-0','0-1'): continue
        s=1 if g.headers['Result']=='1-0' else -1
        b=g.board(); cnt={k:0 for k in B}; started=False; mx=-9999
        for m in g.mainline_moves():
            r=an.get(" ".join(b.fen().split()[:4]))
            if r and r['lines']:
                v=s*(r['lines'][0][1] if b.turn else -r['lines'][0][1])
                if v>=300: started=True
                if started:
                    k='lt300' if v<300 else '300-500' if v<500 else '500-900' if v<900 else '900-1500' if v<1500 else '1500+'
                    cnt[k]+=1
            b.push(m)
        if started:
            for k in B: B[k].append(cnt[k])
    print(w,{k:round(statistics.mean(v),1) for k,v in B.items()},len(B['lt300']))
