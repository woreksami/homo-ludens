import sys, chess, chess.pgn, collections
exec(open(sys.argv[2]).read().split("f = open")[0])
f = open(sys.argv[1]); c = collections.Counter()
while (g := chess.pgn.read_game(f)):
    b = g.board()
    for i, m in enumerate(g.mainline_moves()):
        if 5 <= i//2+1 <= 10:
            elo = int(g.headers["WhiteElo" if b.turn else "BlackElo"])
            if total(b, elo) < 1.5:
                e = book.get(" ".join(b.fen().split()[:3]), {})
                s = sum(e.values()); c["out"] += 1
                c["any>=3"] += s >= 3; c["any>=10"] += s >= 10
        b.push(m)
print(c)
