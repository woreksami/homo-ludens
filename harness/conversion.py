"""How long players take to finish a won game: plies from the first time the
eventual winner is +5 (stays decided) to the end, by ending type."""
import sys, json, collections, statistics as st, chess.pgn
d_dir = sys.argv[1]
ev = {}
for l in open(f"{d_dir}/analysis_50000.jsonl"):
    d = json.loads(l)
    if d.get("lines"): ev[d["fen"]] = d["lines"][0][1]
for name in sys.argv[2:]:
    f = open(f"{d_dir}/{name}.pgn"); res = collections.defaultdict(list); clocks = collections.defaultdict(list)
    while True:
        g = chess.pgn.read_game(f)
        if g is None: break
        r = g.headers["Result"]
        if r not in ("1-0", "0-1"): continue
        win = chess.WHITE if r == "1-0" else chess.BLACK
        b = g.board(); plies = []; first = None
        for i, n in enumerate(g.mainline()):
            b.push(n.move)
            e = ev.get(b.fen())
            if e is None: continue
            we = e if b.turn == win else -e   # from winner's view (e is side-to-move)
            if we >= 500 and first is None: first = i
            if we < 300: first = None
        n = i + 1
        end = "mate" if b.is_checkmate() else g.headers.get("Termination", "?")
        if first is not None:
            res[end].append(n - first)
    print(name, {k: (len(v), round(st.median(v)), round(st.mean(v))) for k, v in res.items()})
