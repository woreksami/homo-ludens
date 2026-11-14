import json, sys, chess
sys.path.insert(0,"harness"); import calibrate
from pathlib import Path
pos=calibrate.sample(Path("reports/anticheat_big2/humans.pgn"),80,200,7)
mv=json.load(open(sys.argv[1]))
bc={k:0 for k in mv}
for i,(_,s,moves,h,_) in enumerate(pos):
    b=chess.Board()
    for x in moves: b.push_uci(x)
    for k in mv: bc[k]+=b.gives_check(chess.Move.from_uci(mv[k][i]))
print("check rate",{k:round(v/len(pos),3) for k,v in bc.items()}, "(human 0.108)")
