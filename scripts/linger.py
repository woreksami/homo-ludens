import sys, json, chess.pgn, statistics
an = {}
for line in open(sys.argv[1] + "/analysis_50000.jsonl"):
    r = json.loads(line); an[" ".join(r["fen"].split()[:4])] = r
for who in ("humans", "homo_ludens"):
    f = open(f"{sys.argv[1]}/{who}.pgn"); out = {500: [], 900: [], 1500: []}
    while (g := chess.pgn.read_game(f)):
        b = g.board(); evs = []
        try:
            for m in g.mainline_moves():
                r = an.get(" ".join(b.fen().split()[:4]))
                v = r["lines"][0][1] if r and r["lines"] else None
                evs.append(None if v is None else (v if b.turn else -v)); b.push(m)
        except Exception: continue
        res = g.headers["Result"]
        if res not in ("1-0", "0-1") or b.is_checkmate() or g.headers.get("Termination") == "Time forfeit": continue
        s = 1 if res == "1-0" else -1
        for th in out:
            fi = next((i for i, v in enumerate(evs) if v is not None and s * v >= th), None)
            if fi is not None: out[th].append(len(evs) - fi)
    print(who, "resigned games: plies from winner's first +5/+9/+15 to resignation:", {k: statistics.median(v) for k, v in out.items()}, len(out[500]))
