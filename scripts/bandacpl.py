import sys, json, chess.pgn, collections, statistics
an = {}
for line in open(sys.argv[1] + "/analysis_50000.jsonl"):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
def val(b):
    r = an.get(" ".join(b.fen().split()[:4]))
    return r["lines"][0][1] if r and r["lines"] else None
for who in ("humans", "homo_ludens"):
    res = collections.defaultdict(list); f = open(f"{sys.argv[1]}/{who}.pgn")
    while (g := chess.pgn.read_game(f)):
        b = g.board()
        try:
            for m in g.mainline_moves():
                elo = int(g.headers["WhiteElo" if b.turn else "BlackElo"]); band = elo // 200 * 200
                v0 = val(b); b.push(m); v1 = val(b)
                if v0 is not None and v1 is not None:
                    res[band].append(max(0, min(1000, v0) - min(1000, -v1)))
        except Exception: pass
    print(who.ljust(12), "  ".join(f"{k}:{statistics.fmean(v):5.1f}" for k, v in sorted(res.items())))
