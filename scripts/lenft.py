import chess.pgn,statistics,sys,re
for w in ('humans','homo_ludens'):
  f=open(sys.argv[1]+'/'+w+'.pgn');L=[];T=0;P=[]
  while (g:=chess.pgn.read_game(f)):
    L.append(len(list(g.mainline_moves())));T+=g.headers.get('Termination')=='Time forfeit'
    b=g.end().board(); P.append(len(b.piece_map()))
  print(sys.argv[1].split('/')[-1],w,round(statistics.mean(L),1),'forfeits',T,'end pieces',statistics.median(P),len(L))
