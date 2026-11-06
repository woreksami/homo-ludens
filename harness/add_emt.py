"""Normalise a clocked PGN for the harness: TimeControl as base+inc, standard
chess only, and [%emt] (seconds spent, from consecutive %clk readings plus the
increment) on every move after the replayed opening.
usage: add_emt.py out.pgn in1.pgn [in2.pgn ...]"""
import re, sys, chess.pgn
CLK = re.compile(r'\[%clk\s+(\d+):(\d+):([\d.]+)\]')
out = open(sys.argv[1], 'w'); n = skipped = 0
for path in sys.argv[2:]:
    f = open(path, errors='replace')
    while True:
        try:
            g = chess.pgn.read_game(f)
        except Exception:
            skipped += 1; continue
        if g is None: break
        h = g.headers; tc = h.get('TimeControl', '')
        if h.get('Variant') or 'FEN' in h or not re.fullmatch(r'\d+(\+\d+)?', tc) or 'WhiteElo' not in h:
            skipped += 1; continue
        if '+' not in tc: tc += '+0'
        h['TimeControl'] = tc
        base, inc = (float(x) for x in tc.split('+'))
        last = [base, base]; ok = True; plies = 0
        for i, node in enumerate(g.mainline()):
            plies += 1
            m = CLK.search(node.comment or '')
            if not m: ok = False; break
            c = int(m.group(1)) * 3600 + int(m.group(2)) * 60 + float(m.group(3))
            emt = max(0.0, last[i % 2] - c + inc) if i >= 2 else None
            last[i % 2] = c
            node.comment = f"[%clk {m.group(1)}:{m.group(2)}:{m.group(3)}]" + (f" [%emt {emt:.1f}]" if emt is not None and i >= 8 else "")
        if not ok or plies < 10:
            skipped += 1; continue
        out.write(str(g) + '\n\n'); n += 1
print(n, 'games written,', skipped, 'skipped')
