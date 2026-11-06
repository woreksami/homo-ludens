"""Eval (loser's view, cp) at the final position of games that ended by resignation, and ply."""
import sys, json, statistics, chess.pgn
d, = sys.argv[1:2]
ev = {}
for l in open(f"{d}/analysis_50000.jsonl"):
    x = json.loads(l)
    if x.get("lines"): ev[x["fen"]] = x["lines"][0][1]
for name in ("humans", "homo_ludens"):
    f = open(f"{d}/{name}.pgn"); vals = []; plies = []; mates = 0; dec = 0
    while (g := chess.pgn.read_game(f)):
        r = g.headers["Result"]
        if r not in ("1-0", "0-1"): continue
        dec += 1
        b = g.end().board()
        if b.is_checkmate(): mates += 1; continue
        t = g.headers.get("Termination", "")
        if "tempo" in t or "time" in t.lower(): continue
        loser = chess.BLACK if r == "1-0" else chess.WHITE
        fen = b.fen()
        if fen not in ev: continue
        e = ev[fen] if b.turn == loser else -ev[fen]
        vals.append(max(-3000, e)); plies.append(b.ply())
    q = statistics.quantiles(vals, n=4)
    print(f"{name}: decided {dec} mates {mates} resigns {len(vals)} eval quartiles {q} median ply {statistics.median(plies)}")
