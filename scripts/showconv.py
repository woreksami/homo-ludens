import sys, json, chess.pgn
an = {}
for line in open(sys.argv[1] + "/analysis_50000.jsonl"):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
f = open(f"{sys.argv[1]}/homo_ludens.pgn"); k = 0
while (g := chess.pgn.read_game(f)):
    b = g.board(); rows = []; first = None
    for m in g.mainline_moves():
        r = an.get(" ".join(b.fen().split()[:4])); v = r["lines"][0][1] if r and r["lines"] else None
        if v is not None and b.turn == chess.BLACK: v = -v
        if first is None and v is not None and abs(v) >= 500: first = len(rows)
        rows.append((b.fullmove_number, b.san(m), v, r["lines"][0][0] if r and r["lines"] else "")); b.push(m)
    if first is not None and len(rows) - first > 70 and int(g.headers["WhiteElo"]) < 1500:
        k += 1
        print(g.headers["WhiteElo"], g.headers["BlackElo"], g.headers["Result"])
        print(" ".join(f"{n}.{s}({v},{bm})" for n, s, v, bm in rows[first:first + 40]))
        if k >= 2: break
