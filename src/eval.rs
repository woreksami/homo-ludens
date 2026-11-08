//! Handcrafted evaluation split into named concepts.
//!
//! Every term is a human chess concept with a name. The full engine weighs all
//! of them at 1.0. A simulated player of a given rating only "knows" some of
//! them, with its own weights and noise (see `mind::persona`). That is the
//! knowledge-gated evaluation from the blueprint (stage 5).

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
    b.sq.iter().map(|&p| PHASE_WEIGHT[piece_type(p) as usize]).sum::<i32>().min(MAX_PHASE)
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

/// Full evaluation broken down by concept (White's point of view).
///
/// The terms and weights mirror harness/texel/features.py; the weights in
/// `eval_params` are tuned on master games.
pub fn breakdown(b: &Board) -> Breakdown {
    let phase = game_phase(b);
    let mut acc = [Acc::default(); N_CONCEPTS];
    // The king's square table counts as piece placement in the middlegame
    // and as king activity in the endgame.
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
                }
                if is_passed(b, s, c) {
                    let rr = if c == WHITE { r } else { 7 - r };
                    if (1..=6).contains(&rr) {
                        let i = (rr - 1) as usize;
                        acc[Concept::PassedPawns as usize].add(sign, 1, (PASSED_MG[i], PASSED_EG[i]));
                    }
                }
            }
            KNIGHT => {
                let n = knight_targets(s).iter().filter(|&&q| own_free(b, q, c)).count() as i32;
                acc[Concept::Mobility as usize].add(sign, n, (MOB_MG[0], MOB_EG[0]));
            }
            BISHOP => {
                bishops[c as usize] += 1;
                acc[Concept::Mobility as usize].add(sign, slider_mobility(b, s, c, &BISHOP_D), (MOB_MG[1], MOB_EG[1]));
            }
            ROOK => {
                acc[Concept::Mobility as usize].add(sign, slider_mobility(b, s, c, &ROOK_D), (MOB_MG[2], MOB_EG[2]));
                let fu = f as usize;
                if pawn_files[c as usize][fu] == 0 {
                    let w = if pawn_files[(c ^ 1) as usize][fu] == 0 { ROOK_OPEN } else { ROOK_HALF_OPEN };
                    acc[Concept::RookFiles as usize].add(sign, 1, w);
                }
            }
            QUEEN => {
                let m = slider_mobility(b, s, c, &BISHOP_D) + slider_mobility(b, s, c, &ROOK_D);
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
    // Threats, outposts, pawn chains, rooks on the seventh, side to move.
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
    }

    let mut t = [0i32; N_CONCEPTS];
    for (i, a) in acc.iter().enumerate() {
        t[i] = a.taper(phase);
    }
    t[Concept::PieceSquares as usize] += king_pst_mg * phase / MAX_PHASE;
    t[Concept::KingActivity as usize] += king_pst_eg * (MAX_PHASE - phase) / MAX_PHASE;
    Breakdown { terms: t }
}

const EVAL_CACHE_SIZE: usize = 1 << 16;

thread_local! {
    /// Recently evaluated positions (hash -> White's score). Searches evaluate
    /// the same positions many times over; the static score never changes.
    static EVAL_CACHE: std::cell::RefCell<Vec<(u64, i32)>> = std::cell::RefCell::new(vec![(0, 0); EVAL_CACHE_SIZE]);
}

/// Evaluation from the side to move's point of view, all concepts at full weight.
pub fn evaluate(b: &Board) -> i32 {
    let i = (b.hash as usize) & (EVAL_CACHE_SIZE - 1);
    let cached = EVAL_CACHE.with(|c| {
        let e = c.borrow()[i];
        if e.0 == b.hash && b.hash != 0 { Some(e.1) } else { None }
    });
    let s = match cached {
        Some(v) => v,
        None => {
            let v = breakdown(b).total();
            EVAL_CACHE.with(|c| c.borrow_mut()[i] = (b.hash, v));
            v
        }
    };
    if b.side == WHITE {
        s
    } else {
        -s
    }
}

#[inline]
fn own_free(b: &Board, sq: u8, c: u8) -> bool {
    let p = b.sq[sq as usize];
    p == 0 || piece_color(p) != c
}

fn slider_mobility(b: &Board, s: u8, c: u8, dirs: &[(i8, i8)]) -> i32 {
    let mut n = 0;
    for &(df, dr) in dirs {
        for q in ray(s, df, dr) {
            let p = b.sq[q as usize];
            if p == 0 {
                n += 1;
            } else {
                if piece_color(p) != c {
                    n += 1;
                }
                break;
            }
        }
    }
    n
}

pub fn is_passed(b: &Board, s: u8, c: u8) -> bool {
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
