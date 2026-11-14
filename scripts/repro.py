import sys, re, subprocess, chess.pgn
pgn, target, bin_ = sys.argv[1], sys.argv[2], sys.argv[3]
f = open(pgn); i = -1
while (g := chess.pgn.read_game(f)):
    i += 1
    if g.end().board().fen() == target: break
h = g.headers; base, inc = (float(x) for x in h["TimeControl"].split("+"))
clocks = [base - 4, base - 4]; moves = []; b = g.board()
for n, node in enumerate(g.mainline()):
    m = re.search(r'%emt ([\d.]+)', node.comment)
    if m: clocks[0 if b.turn else 1] -= float(m.group(1)); clocks[0 if b.turn else 1] += inc
    moves.append(node.move.uci()); b.push(node.move)
elo = int(h["WhiteElo" if b.turn else "BlackElo"])
cmd = f"uci\nsetoption name UCI_Elo value {elo}\nsetoption name Seed value {i*1000+b.ply()}\nsetoption name SimTimeControl value {h['TimeControl']}\nposition startpos moves {' '.join(moves)}\ngo movetime 200 wtime {int(clocks[0]*1000)} btime {int(clocks[1]*1000)} winc {int(inc*1000)} binc {int(inc*1000)}\nquit\n"
print("game", i, "clocks", clocks)
print(subprocess.run([bin_], input=cmd, capture_output=True, text=True).stdout)
