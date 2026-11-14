import sys, json, collections, statistics as st, chess.pgn
d_dir = sys.argv[1]
ev = {}
for l in open(f"{d_dir}/analysis_50000.jsonl"):
    d = json.loads(l)
    if d.get("lines"): ev[d["fen"]] = d["lines"][0]
for name in sys.argv[2:]:
    f = open(f"{d_dir}/{name}.pgn"); S=collections.defaultdict(list)
    while True:
        g = chess.pgn.read_game(f)
        if g is None: break
        r = g.headers["Result"]
        if r not in ("1-0","0-1"): continue
        win = chess.WHITE if r=="1-0" else chess.BLACK
        b = g.board(); decided=False
        for n in g.mainline():
            fen=b.fen(); mover=b.turn; uci=n.move.uci(); chk=b.gives_check(n.move); cap=b.is_capture(n.move)
            b.push(n.move)
            if fen not in ev: continue
            e0=ev[fen][1]; we = e0 if mover==win else -e0
            if we>=500: decided=True
            if we<300: decided=False
            if not decided or b.fen() not in ev: continue
            cpl=max(0,min(1000,e0)+min(1000,ev[b.fen()][1])) if abs(e0)<5000 else None
            side='W' if mover==win else 'L'
            S[side+'top'].append(uci==ev[fen][0])
            S[side+'chk'].append(chk); S[side+'cap'].append(cap)
    print(name, {k: round(sum(v)/len(v),3) for k,v in S.items()}, len(S['Wtop']))
