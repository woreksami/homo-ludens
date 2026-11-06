"""Fast banded opening book from big PGN dumps (reads only the first plies).
Same output format as build_book.py.
usage: build_book_fast.py out.tsv --exclude ex.pgn ... --pgn a.pgn b.pgn ...
Games whose (White, Black, Date) appear in an --exclude file are skipped."""
import re, sys, glob
from collections import defaultdict
from multiprocessing import Pool
import chess

MAX_PLY, BAND, MIN_COUNT = 24, 200, 3
HDR = re.compile(r'\[(\w+) "([^"]*)"\]')
COMMENT = re.compile(r'\{[^}]*\}')
NUM = re.compile(r'\d+\.(\.\.)?')


def games(path):
    buf = []
    for line in open(path, errors='replace'):
        if line.startswith('[Event ') and buf:
            yield ''.join(buf); buf = []
        buf.append(line)
    if buf:
        yield ''.join(buf)


def work(args):
    path, skip = args
    counts = defaultdict(lambda: defaultdict(int)); n = 0
    for text in games(path):
        h = dict(HDR.findall(text))
        if h.get('Variant') or 'FEN' in h or (h.get('White'), h.get('Black'), h.get('Date')) in skip:
            continue
        try:
            elo = [int(h['WhiteElo']), int(h['BlackElo'])]
        except (KeyError, ValueError):
            continue
        mv = NUM.sub(' ', COMMENT.sub(' ', text.split('\n\n', 1)[-1])).split()
        b = chess.Board(); n += 1
        for i, san in enumerate(mv[:MAX_PLY]):
            if san in ('1-0', '0-1', '1/2-1/2', '*'):
                break
            try:
                m = b.parse_san(san)
            except ValueError:
                break
            band = elo[0 if b.turn else 1] // BAND * BAND
            counts[(band, ' '.join(b.fen().split()[:3]))][m.uci()] += 1
            b.push(m)
    return n, {k: dict(v) for k, v in counts.items()}


def keys(path):
    out = set()
    for text in games(path):
        h = dict(HDR.findall(text[:2000]))
        out.add((h.get('White'), h.get('Black'), h.get('Date')))
    return out


if __name__ == '__main__':
    a = sys.argv[1:]; out = a[0]
    ex = a[a.index('--exclude') + 1:a.index('--pgn')] if '--exclude' in a else []
    pgns = [p for g in a[a.index('--pgn') + 1:] for p in glob.glob(g)]
    skip = set().union(*(keys(p) for p in ex)) if ex else set()
    total = defaultdict(lambda: defaultdict(int)); n = 0
    with Pool(2) as pool:
        for k, c in pool.imap_unordered(work, [(p, skip) for p in pgns]):
            n += k
            for key, mv in c.items():
                for u, x in mv.items():
                    total[key][u] += x
    w = 0
    with open(out, 'w') as f:
        for (band, fen), mv in sorted(total.items()):
            if sum(mv.values()) < MIN_COUNT:
                continue
            f.write(f"{band}\t{fen}\t{' '.join(f'{u}:{c}' for u, c in sorted(mv.items(), key=lambda x: -x[1]))}\n"); w += 1
    print(f"{n} games -> {w} entries in {out}")
