"""Convert the Kaggle 'Chess Game Dataset (Lichess)' games.csv (20k games, 2016-17) to PGN.

Keeps rated games only, drops duplicates, maps victory_status to Termination, and
writes TimeControl in seconds (the CSV gives base minutes + increment seconds).
"""
import csv, sys
from pathlib import Path
import chess, chess.pgn

src, out = Path(sys.argv[1]), Path(sys.argv[2])
seen = set(); n = 0
with open(src) as f, open(out, "w") as w:
    for r in csv.DictReader(f):
        if r["rated"].upper() != "TRUE" or r["id"] in seen:
            continue
        seen.add(r["id"])
        try:
            base, inc = r["increment_code"].split("+")
            tc = f"{int(base) * 60}+{int(inc)}"
        except ValueError:
            continue
        g = chess.pgn.Game()
        b = chess.Board(); node = g; ok = True
        for san in r["moves"].split():
            try:
                m = b.parse_san(san)
            except ValueError:
                ok = False; break
            node = node.add_variation(m); b.push(m)
        if not ok:
            continue
        res = {"white": "1-0", "black": "0-1", "draw": "1/2-1/2"}[r["winner"]]
        term = {"outoftime": "Time forfeit", "resign": "Normal", "mate": "Normal", "draw": "Normal"}[r["victory_status"]]
        g.headers.update({"Event": "Rated game", "Site": f"https://lichess.org/{r['id']}", "Date": "????.??.??", "Round": "-",
                          "White": r["white_id"], "Black": r["black_id"], "Result": res, "WhiteElo": r["white_rating"],
                          "BlackElo": r["black_rating"], "TimeControl": tc, "Termination": term, "VictoryStatus": r["victory_status"]})
        print(g, file=w, end="\n\n"); n += 1
print(n, "games", file=sys.stderr)
