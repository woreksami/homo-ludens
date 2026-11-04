"""Linear evaluation features, mirrored exactly by src/eval.rs.

Each term is a signed count (White minus Black). The evaluation is
sum(term * (mg_weight * phase + eg_weight * (24 - phase)) / 24), so tuning is
logistic regression on game results (Texel's method).
"""

from __future__ import annotations

import chess

KINDS = [chess.PAWN, chess.KNIGHT, chess.BISHOP, chess.ROOK, chess.QUEEN, chess.KING]
# Term layout.
MAT = 0                      # 5 terms: P N B R Q
PST = MAT + 5                # 6 x 64, white-relative square (a1 = 0)
MOB = PST + 6 * 64           # 4: N B R Q
DOUBLED = MOB + 4
ISOLATED = DOUBLED + 1
PASSED = ISOLATED + 1        # 6: relative rank 2..7
SHIELD1 = PASSED + 6
SHIELD2 = SHIELD1 + 1
KOPEN = SHIELD2 + 1
KATT = KOPEN + 1
ROOK_OPEN = KATT + 1
ROOK_HALF = ROOK_OPEN + 1
BPAIR = ROOK_HALF + 1
N_TERMS = BPAIR + 1

PHASE_W = {chess.KNIGHT: 1, chess.BISHOP: 1, chess.ROOK: 2, chess.QUEEN: 4}
BISHOP_D = [(1, 1), (1, -1), (-1, 1), (-1, -1)]
ROOK_D = [(1, 0), (-1, 0), (0, 1), (0, -1)]
KNIGHT_D = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)]
KING_D = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)]


def sq_at(f, r):
    return r * 8 + f if 0 <= f < 8 and 0 <= r < 8 else None


def phase(b: chess.Board) -> int:
    return min(24, sum(PHASE_W.get(p.piece_type, 0) for p in b.piece_map().values()))


def features(b: chess.Board) -> tuple[dict[int, float], int]:
    x: dict[int, float] = {}

    def add(t, v):
        x[t] = x.get(t, 0.0) + v

    pm = b.piece_map()
    pawn_files = {chess.WHITE: [0] * 8, chess.BLACK: [0] * 8}
    for s, p in pm.items():
        if p.piece_type == chess.PAWN:
            pawn_files[p.color][s % 8] += 1
    bishops = {chess.WHITE: 0, chess.BLACK: 0}

    def own_free(q, c):
        p = pm.get(q)
        return p is None or p.color != c

    def slider(s, c, dirs):
        n = 0
        f0, r0 = s % 8, s // 8
        for df, dr in dirs:
            f, r = f0 + df, r0 + dr
            while (q := sq_at(f, r)) is not None:
                p = pm.get(q)
                if p is None:
                    n += 1
                else:
                    if p.color != c:
                        n += 1
                    break
                f, r = f + df, r + dr
        return n

    for s, p in pm.items():
        c = p.color
        sign = 1 if c == chess.WHITE else -1
        k = p.piece_type
        rel = s if c == chess.WHITE else s ^ 56
        f, r = s % 8, s // 8
        if k != chess.KING:
            add(MAT + k - 1, sign)
        add(PST + (k - 1) * 64 + rel, sign)
        if k == chess.PAWN:
            left = pawn_files[c][f - 1] if f > 0 else 0
            right = pawn_files[c][f + 1] if f < 7 else 0
            if left == 0 and right == 0:
                add(ISOLATED, sign)
            # passed: no enemy pawn ahead on this or adjacent files
            passed = True
            for ff in (f - 1, f, f + 1):
                if not 0 <= ff < 8:
                    continue
                rng = range(r + 1, 8) if c == chess.WHITE else range(0, r)
                for rr in rng:
                    q = pm.get(rr * 8 + ff)
                    if q and q.piece_type == chess.PAWN and q.color != c:
                        passed = False
                        break
                if not passed:
                    break
            if passed:
                rr = r if c == chess.WHITE else 7 - r
                if 1 <= rr <= 6:
                    add(PASSED + rr - 1, sign)
        elif k == chess.KNIGHT:
            n = sum(1 for df, dr in KNIGHT_D if (q := sq_at(f + df, r + dr)) is not None and own_free(q, c))
            add(MOB + 0, sign * n)
        elif k == chess.BISHOP:
            bishops[c] += 1
            add(MOB + 1, sign * slider(s, c, BISHOP_D))
        elif k == chess.ROOK:
            add(MOB + 2, sign * slider(s, c, ROOK_D))
            if pawn_files[c][f] == 0:
                add(ROOK_OPEN if pawn_files[not c][f] == 0 else ROOK_HALF, sign)
        elif k == chess.QUEEN:
            add(MOB + 3, sign * (slider(s, c, BISHOP_D) + slider(s, c, ROOK_D)))
        elif k == chess.KING:
            fwd = 1 if c == chess.WHITE else -1
            for df in (-1, 0, 1):
                ff = f + df
                if not 0 <= ff < 8:
                    continue
                for dist in (1, 2):
                    rr = r + fwd * dist
                    if 0 <= rr < 8:
                        q = pm.get(rr * 8 + ff)
                        if q and q.piece_type == chess.PAWN and q.color == c:
                            add(SHIELD1 if dist == 1 else SHIELD2, sign)
                            break
                if pawn_files[c][ff] == 0:
                    add(KOPEN, sign)
            att = sum(1 for df, dr in KING_D if (q := sq_at(f + df, r + dr)) is not None and b.is_attacked_by(not c, q))
            add(KATT, sign * att * att)
    for c in (chess.WHITE, chess.BLACK):
        sign = 1 if c == chess.WHITE else -1
        extra = sum(n - 1 for n in pawn_files[c] if n > 1)
        if extra:
            add(DOUBLED, sign * extra)
        if bishops[c] >= 2:
            add(BPAIR, sign)
    return x, phase(b)
