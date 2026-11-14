//! Hand-written eval, split into named concepts. The full engine uses all of them;
//! a persona only knows some (see mind::persona).

use crate::board::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Concept {
    Material = 0,
    PieceSquares,
    Mobility,
    PawnStructure,
    PassedPawns,
    KingSafety,
    RookFiles,
    BishopPair,
    KingActivity,
}

pub const N_CONCEPTS: usize = 9;

pub const CONCEPT_NAMES: [&str; N_CONCEPTS] = [
    "material",
    "piece placement",
    "mobility",
    "pawn structure",
    "passed pawns",
    "king safety",
    "rooks on open files",
    "bishop pair",
    "king activity (endgame)",
];

pub const PIECE_VALUE: [i32; 7] = [0, 100, 320, 330, 500, 900, 0];

/// Per-concept score breakdown, in centipawns from White's point of view.
#[derive(Clone, Copy, Debug, Default)]
pub struct Breakdown {
    pub terms: [i32; N_CONCEPTS],
}

impl Breakdown {
    pub fn total(&self) -> i32 {
        self.terms.iter().sum()
    }

    /// Weighted sum, e.g. using only the concepts a persona knows.
    pub fn weighted(&self, w: &[f32; N_CONCEPTS]) -> i32 {
        self.terms.iter().zip(w.iter()).map(|(&t, &k)| (t as f32 * k) as i32).sum()
    }
}

use crate::eval_params::*;

const PHASE_WEIGHT: [i32; 7] = [0, 0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

/// 0 = pure endgame, MAX_PHASE = full middlegame.
pub fn game_phase(b: &Board) -> i32 {
    (PAWN..=KING)
        .map(|k| PHASE_WEIGHT[k as usize] * (b.pieces(WHITE, k) | b.pieces(BLACK, k)).count_ones() as i32)
        .sum::<i32>()
        .min(MAX_PHASE)
}

/// Middlegame and endgame accumulators for one concept.
#[derive(Clone, Copy, Default)]
struct Acc {
    mg: i32,
    eg: i32,
}

impl Acc {
    #[inline]
    fn add(&mut self, sign: i32, n: i32, w: (i32, i32)) {
        self.mg += sign * n * w.0;
        self.eg += sign * n * w.1;
    }
    #[inline]
    fn taper(self, phase: i32) -> i32 {
        (self.mg * phase + self.eg * (MAX_PHASE - phase)) / MAX_PHASE
    }
}

/// Full eval by concept, white POV. Terms mirror harness/texel/features.py.
/// (bitboard version; `breakdown_ref` in the tests is the slow original it must match)
pub fn breakdown(b: &Board) -> Breakdown {
    let phase = game_phase(b);
    let mut acc = [Acc::default(); N_CONCEPTS];
    let mut king_pst_mg = 0;
    let mut king_pst_eg = 0;

    let pawns = [b.pieces(WHITE, PAWN), b.pieces(BLACK, PAWN)];
    let mut pawn_files = [[0u8; 8]; 2];
    for c in 0..2 {
        for f in 0..8 {
            pawn_files[c][f] = (pawns[c] & FILE << f).count_ones() as u8;
        }
    }
    let patt = [
        ((pawns[0] & !FILE) << 7) | ((pawns[0] & !(FILE << 7)) << 9),
        ((pawns[1] & !FILE) >> 9) | ((pawns[1] & !(FILE << 7)) >> 7),
    ];
    let own = [b.color_mask(WHITE), b.color_mask(BLACK)];
    let big = [b.pieces(WHITE, ROOK) | b.pieces(WHITE, QUEEN), b.pieces(BLACK, ROOK) | b.pieces(BLACK, QUEEN)];
    let mut bishops = [0; 2];

    let mut left = b.occ;
    while left != 0 {
        let s = left.trailing_zeros() as u8;
        left &= left - 1;
        let p = b.sq[s as usize];
        let c = piece_color(p);
        let ci = c as usize;
        let sign = if c == WHITE { 1 } else { -1 };
        let kind = piece_type(p);
        let k = (kind - 1) as usize;
        let rel = if c == WHITE { s } else { s ^ 56 } as usize;
        let (f, r) = (file_of(s), rank_of(s));

        if kind != KING {
            acc[Concept::Material as usize].add(sign, 1, (MAT_MG[k], MAT_EG[k]));
            acc[Concept::PieceSquares as usize].add(sign, 1, (PST_MG[k][rel], PST_EG[k][rel]));
        }
        match kind {
            PAWN => {
                let fu = f as usize;
                let lf = if fu > 0 { pawn_files[ci][fu - 1] } else { 0 };
                let rf = if fu < 7 { pawn_files[ci][fu + 1] } else { 0 };
                if lf == 0 && rf == 0 {
                    acc[Concept::PawnStructure as usize].add(sign, 1, ISOLATED);
                } else {
                    // backward: no pawn beside/behind it and its push square is hit by an enemy pawn
                    let behind = if c == WHITE { ranks_upto(r) } else { !ranks_upto(r - 1) };
                    if pawns[ci] & adjacent(f) & behind == 0 {
                        let stop = if c == WHITE { r + 1 } else { r - 1 };
                        if (0..8).contains(&stop) && patt[ci ^ 1] >> (stop * 8 + f) & 1 != 0 {
                            acc[Concept::PawnStructure as usize].add(sign, 1, BACKWARD);
                        }
                    }
                }
                if is_passed(b, s, c) {
                    let rr = if c == WHITE { r } else { 7 - r };
                    if (1..=6).contains(&rr) {
                        let i = (rr - 1) as usize;
                        acc[Concept::PassedPawns as usize].add(sign, 1, (PASSED_MG[i], PASSED_EG[i]));
                    }
                }
                let n = (pawn_attacks(c, s) & own[ci ^ 1] & !pawns[ci ^ 1] & !b.pieces(c ^ 1, KING)).count_ones() as i32;
                if n > 0 {
                    acc[Concept::Mobility as usize].add(sign, n, PAWN_THREAT);
                }
                if patt[ci] >> s & 1 != 0 {
                    acc[Concept::PawnStructure as usize].add(sign, 1, SUPPORTED);
                }
            }
            KNIGHT => {
                let t = knight_mask(s);
                let n = (t & !own[ci] & !patt[ci ^ 1]).count_ones() as i32;
                acc[Concept::Mobility as usize].add(sign, n, (MOB_MG[0], MOB_EG[0]));
                let hits = (t & big[ci ^ 1]).count_ones() as i32;
                if hits > 0 {
                    acc[Concept::Mobility as usize].add(sign, hits, MINOR_THREAT);
                }
                let rr = if c == WHITE { r } else { 7 - r };
                if (3..=5).contains(&rr) && patt[ci] >> s & 1 != 0 {
                    let ahead = if c == WHITE { !ranks_upto(r) } else { ranks_upto(r - 1) };
                    if pawns[ci ^ 1] & adjacent(f) & ahead == 0 {
                        acc[Concept::PieceSquares as usize].add(sign, 1, OUTPOST);
                    }
                }
            }
            BISHOP => {
                bishops[ci] += 1;
                let (m, hits) = slider_info(b, s, 4..8, own[ci], big[ci ^ 1], patt[ci ^ 1]);
                acc[Concept::Mobility as usize].add(sign, m, (MOB_MG[1], MOB_EG[1]));
                if hits > 0 {
                    acc[Concept::Mobility as usize].add(sign, hits, MINOR_THREAT);
                }
            }
            ROOK => {
                acc[Concept::Mobility as usize].add(sign, slider_info(b, s, 0..4, own[ci], big[ci ^ 1], patt[ci ^ 1]).0, (MOB_MG[2], MOB_EG[2]));
                let fu = f as usize;
                if pawn_files[ci][fu] == 0 {
                    let w = if pawn_files[ci ^ 1][fu] == 0 { ROOK_OPEN } else { ROOK_HALF_OPEN };
                    acc[Concept::RookFiles as usize].add(sign, 1, w);
                }
                if (if c == WHITE { r } else { 7 - r }) == 6 {
                    acc[Concept::RookFiles as usize].add(sign, 1, ROOK_SEVENTH);
                }
            }
            QUEEN => {
                let m = slider_info(b, s, 0..8, own[ci], big[ci ^ 1], patt[ci ^ 1]).0;
                acc[Concept::Mobility as usize].add(sign, m, (MOB_MG[3], MOB_EG[3]));
            }
            KING => {
                king_pst_mg += sign * PST_MG[5][rel];
                king_pst_eg += sign * PST_EG[5][rel];
                king_safety(b, s, c, &pawn_files, sign, &mut acc[Concept::KingSafety as usize]);
            }
            _ => {}
        }
    }
    acc[Concept::PieceSquares as usize].add(if b.side == WHITE { 1 } else { -1 }, 1, TEMPO);
    for c in [WHITE, BLACK] {
        let sign = if c == WHITE { 1 } else { -1 };
        let extra: i32 = pawn_files[c as usize].iter().map(|&n| (n as i32 - 1).max(0)).sum();
        if extra > 0 {
            acc[Concept::PawnStructure as usize].add(sign, extra, DOUBLED);
        }
        if bishops[c as usize] >= 2 {
            acc[Concept::BishopPair as usize].add(sign, 1, BISHOP_PAIR);
        }
        let rights = if c == WHITE { WK_CASTLE | WQ_CASTLE } else { BK_CASTLE | BQ_CASTLE };
        let n_rights = (b.castling & rights).count_ones() as i32;
        if n_rights > 0 {
            acc[Concept::KingSafety as usize].add(sign, n_rights, CASTLE_RIGHTS);
        }
        let base = if c == WHITE { 0usize } else { 56 };
        let home = [(1, KNIGHT), (6, KNIGHT), (2, BISHOP), (5, BISHOP)]
            .iter()
            .filter(|&&(f, t)| b.sq[base + f] == make_piece(c, t))
            .count() as i32;
        if home > 0 {
            acc[Concept::PieceSquares as usize].add(sign, home, UNDEVELOPED);
        }
    }

    let mut t = [0i32; N_CONCEPTS];
    for (i, a) in acc.iter().enumerate() {
        t[i] = a.taper(phase);
    }
    t[Concept::PieceSquares as usize] += king_pst_mg * phase / MAX_PHASE;
    t[Concept::KingActivity as usize] += king_pst_eg * (MAX_PHASE - phase) / MAX_PHASE;
    Breakdown { terms: t }
}

const FILE: u64 = 0x0101_0101_0101_0101;

fn adjacent(f: i8) -> u64 {
    (if f > 0 { FILE << (f - 1) } else { 0 }) | (if f < 7 { FILE << (f + 1) } else { 0 })
}

/// every square on rank `r` or below (empty for r < 0)
fn ranks_upto(r: i8) -> u64 {
    if r < 0 { 0 } else if r >= 7 { !0 } else { (1u64 << (8 * (r + 1))) - 1 }
}

const EVAL_CACHE_SIZE: usize = 1 << 16;

thread_local! {
    // hash -> breakdown. the mind asks for the same positions over and over
    static EVAL_CACHE: std::cell::RefCell<Vec<(u64, Breakdown)>> = std::cell::RefCell::new(vec![(0, Breakdown { terms: [0; N_CONCEPTS] }); EVAL_CACHE_SIZE]);
}

pub fn cached_breakdown(b: &Board) -> Breakdown {
    let i = (b.hash as usize) & (EVAL_CACHE_SIZE - 1);
    if b.hash != 0 {
        if let Some(bd) = EVAL_CACHE.with(|c| {
            let e = &c.borrow()[i];
            (e.0 == b.hash).then_some(e.1)
        }) {
            return bd;
        }
    }
    let bd = breakdown(b);
    EVAL_CACHE.with(|c| c.borrow_mut()[i] = (b.hash, bd));
    bd
}

/// Evaluation from the side to move's point of view, all concepts at full weight.
pub fn evaluate(b: &Board) -> i32 {
    let s = cached_breakdown(b).total();
    if b.side == WHITE { s } else { -s }
}

/// Safe squares a slider reaches (not covered by enemy pawns), and how many enemy
/// rooks/queens it attacks.
fn slider_info(b: &Board, s: u8, dirs: std::ops::Range<usize>, own: u64, big_enemy: u64, unsafe_sq: u64) -> (i32, i32) {
    let (mut reach, mut hits) = (0u64, 0);
    for d in dirs {
        let ray = ray_mask(d, s);
        match first_hit(d, s, b.occ) {
            Some(q) => {
                reach |= ray ^ ray_mask(d, q);
                hits += (big_enemy >> q & 1) as i32;
            }
            None => reach |= ray,
        }
    }
    ((reach & !own & !unsafe_sq).count_ones() as i32, hits)
}

pub fn is_passed(b: &Board, s: u8, c: u8) -> bool {
    let (f, r) = (file_of(s), rank_of(s));
    let files = adjacent(f) | FILE << f;
    let ahead = if c == WHITE { !ranks_upto(r) } else { ranks_upto(r - 1) };
    b.pieces(c ^ 1, PAWN) & files & ahead == 0
}

fn king_safety(b: &Board, k: u8, c: u8, pawn_files: &[[u8; 8]; 2], sign: i32, acc: &mut Acc) {
    let f = file_of(k);
    let r = rank_of(k);
    let fwd: i8 = if c == WHITE { 1 } else { -1 };
    let own_pawn = make_piece(c, PAWN);
    for df in -1..=1i8 {
        let ff = f + df;
        if !(0..8).contains(&ff) {
            continue;
        }
        for dist in 1..=2i8 {
            let rr = r + fwd * dist;
            if (0..8).contains(&rr) && b.sq[(rr * 8 + ff) as usize] == own_pawn {
                acc.add(sign, 1, if dist == 1 { SHIELD1 } else { SHIELD2 });
                break;
            }
        }
        if pawn_files[c as usize][ff as usize] == 0 {
            acc.add(sign, 1, KING_OPEN_FILE);
        }
    }
    let attacked = king_targets(k).iter().filter(|&&q| b.attacked(q, c ^ 1)).count() as i32;
    acc.add(sign, attacked * attacked, KING_ATTACK);
}

#[cfg(test)]
mod tests {
    use super::*;

    // the original square-by-square eval, kept to check the fast one against
    /// Full eval by concept, white POV. Terms mirror harness/texel/features.py.
    pub fn breakdown_ref(b: &Board) -> Breakdown {
        let phase = game_phase_ref(b);
        let mut acc = [Acc::default(); N_CONCEPTS];
        // king table = placement in the mg, activity in the eg
        let mut king_pst_mg = 0;
        let mut king_pst_eg = 0;

        let mut pawn_files = [[0u8; 8]; 2];
        for s in 0..64u8 {
            let p = b.sq[s as usize];
            if piece_type(p) == PAWN {
                pawn_files[piece_color(p) as usize][file_of(s) as usize] += 1;
            }
        }
        let mut bishops = [0; 2];
        let mut patt = [0u64; 2];
        for s in 0..64u8 {
            let p = b.sq[s as usize];
            if p != 0 && piece_type(p) == PAWN {
                let c = piece_color(p);
                let (f, r) = (file_of(s), rank_of(s));
                let r2 = if c == WHITE { r + 1 } else { r - 1 };
                for df in [-1i8, 1] {
                    if let Some(q) = sq_at(f + df, r2) {
                        patt[c as usize] |= 1u64 << q;
                    }
                }
            }
        }

        for s in 0..64u8 {
            let p = b.sq[s as usize];
            if p == 0 {
                continue;
            }
            let c = piece_color(p);
            let sign = if c == WHITE { 1 } else { -1 };
            let kind = piece_type(p);
            let k = (kind - 1) as usize;
            let rel = if c == WHITE { s } else { s ^ 56 } as usize;
            let (f, r) = (file_of(s), rank_of(s));

            if kind != KING {
                acc[Concept::Material as usize].add(sign, 1, (MAT_MG[k], MAT_EG[k]));
                acc[Concept::PieceSquares as usize].add(sign, 1, (PST_MG[k][rel], PST_EG[k][rel]));
            }
            match kind {
                PAWN => {
                    let fu = f as usize;
                    let left = if fu > 0 { pawn_files[c as usize][fu - 1] } else { 0 };
                    let right = if fu < 7 { pawn_files[c as usize][fu + 1] } else { 0 };
                    if left == 0 && right == 0 {
                        acc[Concept::PawnStructure as usize].add(sign, 1, ISOLATED);
                    } else {
                        // backward: no pawn beside/behind it and its push square is hit by an enemy pawn
                        let fwd: i8 = if c == WHITE { 1 } else { -1 };
                        let support = [f - 1, f + 1].iter().any(|&ff| {
                            (0..8i8).any(|rr| (if c == WHITE { rr <= r } else { rr >= r }) && sq_at(ff, rr).map_or(false, |q| b.sq[q as usize] == make_piece(c, PAWN)))
                        });
                        if !support {
                            if let Some(stop) = sq_at(f, r + fwd) {
                                if patt[(c ^ 1) as usize] & (1u64 << stop) != 0 {
                                    acc[Concept::PawnStructure as usize].add(sign, 1, BACKWARD);
                                }
                            }
                        }
                    }
                    if is_passed_ref(b, s, c) {
                        let rr = if c == WHITE { r } else { 7 - r };
                        if (1..=6).contains(&rr) {
                            let i = (rr - 1) as usize;
                            acc[Concept::PassedPawns as usize].add(sign, 1, (PASSED_MG[i], PASSED_EG[i]));
                        }
                    }
                }
                KNIGHT => {
                    let safe = patt[(c ^ 1) as usize];
                    let n = knight_targets(s).iter().filter(|&&q| own_free(b, q, c) && safe & (1u64 << q) == 0).count() as i32;
                    acc[Concept::Mobility as usize].add(sign, n, (MOB_MG[0], MOB_EG[0]));
                    let hits = knight_targets(s).iter().filter(|&&q| big_enemy(b, q, c)).count() as i32;
                    if hits > 0 {
                        acc[Concept::Mobility as usize].add(sign, hits, MINOR_THREAT);
                    }
                }
                BISHOP => {
                    bishops[c as usize] += 1;
                    let (m, hits) = slider_info_ref(b, s, c, &BISHOP_D, patt[(c ^ 1) as usize]);
                    acc[Concept::Mobility as usize].add(sign, m, (MOB_MG[1], MOB_EG[1]));
                    if hits > 0 {
                        acc[Concept::Mobility as usize].add(sign, hits, MINOR_THREAT);
                    }
                }
                ROOK => {
                    acc[Concept::Mobility as usize].add(sign, slider_info_ref(b, s, c, &ROOK_D, patt[(c ^ 1) as usize]).0, (MOB_MG[2], MOB_EG[2]));
                    let fu = f as usize;
                    if pawn_files[c as usize][fu] == 0 {
                        let w = if pawn_files[(c ^ 1) as usize][fu] == 0 { ROOK_OPEN } else { ROOK_HALF_OPEN };
                        acc[Concept::RookFiles as usize].add(sign, 1, w);
                    }
                }
                QUEEN => {
                    let safe = patt[(c ^ 1) as usize];
                    let m = slider_info_ref(b, s, c, &BISHOP_D, safe).0 + slider_info_ref(b, s, c, &ROOK_D, safe).0;
                    acc[Concept::Mobility as usize].add(sign, m, (MOB_MG[3], MOB_EG[3]));
                }
                KING => {
                    king_pst_mg += sign * PST_MG[5][rel];
                    king_pst_eg += sign * PST_EG[5][rel];
                    king_safety(b, s, c, &pawn_files, sign, &mut acc[Concept::KingSafety as usize]);
                }
                _ => {}
            }
        }
        // threats, outposts, chains, rook on 7th, tempo
        let pawn_at = |f: i8, r: i8, c: u8| -> bool {
            (0..8).contains(&f) && (0..8).contains(&r) && b.sq[(r * 8 + f) as usize] == make_piece(c, PAWN)
        };
        for s in 0..64u8 {
            let p = b.sq[s as usize];
            if p == 0 {
                continue;
            }
            let c = piece_color(p);
            let sign = if c == WHITE { 1 } else { -1 };
            let (f, r) = (file_of(s), rank_of(s));
            let fwd: i8 = if c == WHITE { 1 } else { -1 };
            let rr = if c == WHITE { r } else { 7 - r };
            match piece_type(p) {
                PAWN => {
                    for df in [-1i8, 1] {
                        if let Some(q) = sq_at(f + df, r + fwd) {
                            let t = b.sq[q as usize];
                            if t != 0 && piece_color(t) != c && piece_type(t) != PAWN && piece_type(t) != KING {
                                acc[Concept::Mobility as usize].add(sign, 1, PAWN_THREAT);
                            }
                        }
                    }
                    if pawn_at(f - 1, r - fwd, c) || pawn_at(f + 1, r - fwd, c) {
                        acc[Concept::PawnStructure as usize].add(sign, 1, SUPPORTED);
                    }
                }
                KNIGHT if (3..=5).contains(&rr) => {
                    if pawn_at(f - 1, r - fwd, c) || pawn_at(f + 1, r - fwd, c) {
                        let chase = [f - 1, f + 1].iter().any(|&ff| (0..8i8).any(|r2| (if c == WHITE { r2 > r } else { r2 < r }) && pawn_at(ff, r2, c ^ 1)));
                        if !chase {
                            acc[Concept::PieceSquares as usize].add(sign, 1, OUTPOST);
                        }
                    }
                }
                ROOK if rr == 6 => acc[Concept::RookFiles as usize].add(sign, 1, ROOK_SEVENTH),
                _ => {}
            }
        }
        acc[Concept::PieceSquares as usize].add(if b.side == WHITE { 1 } else { -1 }, 1, TEMPO);
        for c in [WHITE, BLACK] {
            let sign = if c == WHITE { 1 } else { -1 };
            let extra: i32 = pawn_files[c as usize].iter().map(|&n| (n as i32 - 1).max(0)).sum();
            if extra > 0 {
                acc[Concept::PawnStructure as usize].add(sign, extra, DOUBLED);
            }
            if bishops[c as usize] >= 2 {
                acc[Concept::BishopPair as usize].add(sign, 1, BISHOP_PAIR);
            }
            let rights = if c == WHITE { WK_CASTLE | WQ_CASTLE } else { BK_CASTLE | BQ_CASTLE };
            let n_rights = (b.castling & rights).count_ones() as i32;
            if n_rights > 0 {
                acc[Concept::KingSafety as usize].add(sign, n_rights, CASTLE_RIGHTS);
            }
            let base = if c == WHITE { 0usize } else { 56 };
            let home = [(1, KNIGHT), (6, KNIGHT), (2, BISHOP), (5, BISHOP)]
                .iter()
                .filter(|&&(f, t)| b.sq[base + f] == make_piece(c, t))
                .count() as i32;
            if home > 0 {
                acc[Concept::PieceSquares as usize].add(sign, home, UNDEVELOPED);
            }
        }

        let mut t = [0i32; N_CONCEPTS];
        for (i, a) in acc.iter().enumerate() {
            t[i] = a.taper(phase);
        }
        t[Concept::PieceSquares as usize] += king_pst_mg * phase / MAX_PHASE;
        t[Concept::KingActivity as usize] += king_pst_eg * (MAX_PHASE - phase) / MAX_PHASE;
        Breakdown { terms: t }
    }


    fn is_passed_ref(b: &Board, s: u8, c: u8) -> bool {
        let f = file_of(s);
        let r = rank_of(s);
        let enemy = make_piece(c ^ 1, PAWN);
        for df in -1..=1i8 {
            let ff = f + df;
            if !(0..8).contains(&ff) {
                continue;
            }
            let (lo, hi) = if c == WHITE { (r + 1, 8) } else { (0, r) };
            for rr in lo..hi {
                if b.sq[(rr * 8 + ff) as usize] == enemy {
                    return false;
                }
            }
        }
        true
    }

    fn game_phase_ref(b: &Board) -> i32 {
        b.sq.iter().map(|&p| PHASE_WEIGHT[piece_type(p) as usize]).sum::<i32>().min(MAX_PHASE)
    }

    fn slider_info_ref(b: &Board, s: u8, c: u8, dirs: &[(i8, i8)], unsafe_sq: u64) -> (i32, i32) {
        let (mut n, mut hits) = (0, 0);
        for &(df, dr) in dirs {
            for q in ray(s, df, dr) {
                let p = b.sq[q as usize];
                let ok = unsafe_sq & (1u64 << q) == 0;
                if p == 0 {
                    n += ok as i32;
                } else {
                    if piece_color(p) != c {
                        n += ok as i32;
                        hits += big_enemy(b, q, c) as i32;
                    }
                    break;
                }
            }
        }
        (n, hits)
    }

    fn own_free(b: &Board, sq: u8, c: u8) -> bool {
        let p = b.sq[sq as usize];
        p == 0 || piece_color(p) != c
    }

    fn big_enemy(b: &Board, q: u8, c: u8) -> bool {
        let p = b.sq[q as usize];
        p != 0 && piece_color(p) != c && (piece_type(p) == ROOK || piece_type(p) == QUEEN)
    }

    #[test]
    fn fast_eval_matches_reference() {
        let fens = [
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
            "r1bq1rk1/pp2bppp/2n1pn2/3p4/2PP4/2NBPN2/PP3PPP/R1BQK2R w KQ - 0 8",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ];
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut n = 0;
        for fen in fens {
            for _ in 0..300 {
                let mut b = Board::from_fen(fen).unwrap();
                for _ in 0..80 {
                    assert_eq!(breakdown(&b).terms, breakdown_ref(&b).terms, "{}", b.to_fen());
                    assert_eq!(game_phase(&b), game_phase_ref(&b));
                    n += 1;
                    let moves = b.legal_moves();
                    if moves.is_empty() {
                        break;
                    }
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    b = b.make(moves[(seed % moves.len() as u64) as usize]);
                }
            }
        }
        assert!(n > 50_000);
    }

    #[test]
    fn start_is_balanced() {
        // Only the side-to-move bonus separates the sides.
        assert_eq!(evaluate(&Board::startpos()), TEMPO.0);
    }

    #[test]
    fn symmetric() {
        // Mirrored positions must evaluate to the same score for the side to move.
        let a = Board::from_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3").unwrap();
        let b = Board::from_fen("rnbqkb1r/pppp1ppp/5n2/4p3/4P3/2N5/PPPP1PPP/R1BQKBNR b KQkq - 2 3").unwrap();
        assert_eq!(evaluate(&a), evaluate(&b));
    }

    #[test]
    fn extra_queen_wins() {
        let b = Board::from_fen("4k3/8/8/8/8/8/8/3QK3 w - - 0 1").unwrap();
        assert!(evaluate(&b) > 800);
    }
}
