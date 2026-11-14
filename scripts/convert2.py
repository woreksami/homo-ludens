import sys, json, chess.pgn, statistics
an = {}
for line in open(sys.argv[1] + "/analysis_50000.jsonl"):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
for who in ("humans", "homo_ludens"):
    f = open(f"{sys.argv[1]}/{who}.pgn"); d = {"mate": [], "resign": [], "other": []}; t515 = []; elo = []
    while (g := chess.pgn.read_game(f)):
        b = g.board(); n = 0; first = None; f15 = None
        try:
            for m in g.mainline_moves():
                r = an.get(" ".join(b.fen().split()[:4]))
                if r and r["lines"]:
                    v = abs(r["lines"][0][1])
                    if first is None and v >= 500: first = n
                    if first is not None and f15 is None and v >= 1500: f15 = n
                b.push(m); n += 1
        except Exception: continue
        if first is None: continue
        kind = "mate" if b.is_checkmate() else ("resign" if g.headers["Result"] != "1/2-1/2" else "other")
        d[kind].append(n - first)
        if f15: t515.append(f15 - first); elo.append(n - f15)
    print(who, {k: (len(v), statistics.median(v) if v else 0) for k, v in d.items()}, "| +5->+15:", statistics.median(t515), " +15->end:", statistics.median(elo))
