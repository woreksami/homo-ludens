import sys, json, chess.pgn, statistics
an = {}
for line in open(sys.argv[1] + "/analysis_50000.jsonl"):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
for who in ("humans", "homo_ludens"):
    f = open(f"{sys.argv[1]}/{who}.pgn"); pre, post, never = [], [], 0
    while (g := chess.pgn.read_game(f)):
        b = g.board(); n = 0; first = None
        try:
            for m in g.mainline_moves():
                r = an.get(" ".join(b.fen().split()[:4]))
                if first is None and r and r["lines"] and abs(r["lines"][0][1]) >= 500: first = n
                b.push(m); n += 1
        except Exception: continue
        if first is None: never += 1
        else: pre.append(first); post.append(n - first)
    print(who, "plies to +-5:", statistics.median(pre), " plies after:", statistics.median(post), " never:", never)
