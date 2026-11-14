import sys, json, chess, chess.pgn, statistics
an = {}
for line in open(sys.argv[1] + "/analysis_50000.jsonl"):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
def val(b):
    r = an.get(" ".join(b.fen().split()[:4])); return r["lines"][0][1] if r and r["lines"] else None
for who in ("humans", "homo_ludens"):
    f = open(f"{sys.argv[1]}/{who}.pgn"); win, lose = [], []; wtop = []
    while (g := chess.pgn.read_game(f)):
        b = g.board()
        try:
            for m in g.mainline_moves():
                v0 = val(b); r = an.get(" ".join(b.fen().split()[:4])); b2 = b.copy(); b2.push(m); v1 = val(b2)
                if v0 is not None and v1 is not None and 400 <= abs(v0) <= 1200:
                    loss = max(0, v0 - (-v1)) if abs(v1) < 3000 else 0
                    (win if v0 > 0 else lose).append(min(loss, 1000))
                    if v0 > 0: wtop.append(m.uci() == r["lines"][0][0])
                b.push(m)
        except Exception: pass
    print(who, "winning side ACPL", round(statistics.fmean(win),1), "top", round(statistics.fmean(wtop),2), "n", len(win), "| losing side ACPL", round(statistics.fmean(lose),1), "n", len(lose))
