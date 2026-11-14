import sys, chess, chess.pgn, collections
book = collections.defaultdict(dict)
for line in open("data/book.tsv"):
    band, fen, mv = line.rstrip("\n").split("\t")
    for x in mv.split():
        u, c = x.split(":"); book[fen][(int(band), u)] = int(c)
def total(b, elo):
    e = book.get(" ".join(b.fen().split()[:3]), {})
    import math
    return sum(math.exp(-(((band+100)-elo)/200)**2/2)*c for (band,u),c in e.items())
f = open(sys.argv[1]); hit = collections.Counter(); n = collections.Counter()
while (g := chess.pgn.read_game(f)):
    hl = "homo" in g.headers.get("White","").lower() or "hl" in g.headers.get("White","").lower()
    b = g.board()
    for i, m in enumerate(g.mainline_moves()):
        mv = i//2 + 1
        if 5 <= mv <= 10:
            col = b.turn
            elo = int(g.headers["WhiteElo" if col else "BlackElo"])
            band = elo//200*200
            n[band] += 1; hit[band] += total(b, elo) >= 1.5
        b.push(m)
for k in sorted(n): print(k, n[k], round(hit[k]/n[k],2))
