import sys, json, chess.pgn, statistics
an = {}
for line in open(sys.argv[1] + "/analysis_50000.jsonl"):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
for who in ("humans", "homo_ludens"):
    f = open(f"{sys.argv[1]}/{who}.pgn"); held = back = 0; firsts = []; lens = []
    while (g := chess.pgn.read_game(f)):
        b = g.board(); evs = []
        try:
            for m in g.mainline_moves():
                r = an.get(" ".join(b.fen().split()[:4]))
                v = r["lines"][0][1] if r and r["lines"] else None
                evs.append(None if v is None else (v if b.turn else -v)); b.push(m)
        except Exception: continue
        lens.append(len(evs))
        fi = next((i for i, v in enumerate(evs) if v is not None and abs(v) >= 500), None)
        if fi is None: continue
        s = 1 if evs[fi] > 0 else -1; firsts.append(fi)
        res = g.headers["Result"]; won = (res == "1-0" and s > 0) or (res == "0-1" and s < 0)
        held += won; back += not won
    print(who, "first +-5 median ply", statistics.median(firsts), "converted", held, "not", back, "games", len(lens), "median len", statistics.median(lens))
