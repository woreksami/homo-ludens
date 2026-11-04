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

// Tables are written rank 8 first (as seen from White). Index for a White
// piece on `sq` is (7 - rank) * 8 + file; for Black it is rank * 8 + file.
#[rustfmt::skip]
const PST_PAWN: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
    50, 50, 50, 50, 50, 50, 50, 50,
    10, 10, 20, 30, 30, 20, 10, 10,
     5,  5, 10, 25, 25, 10,  5,  5,
     0,  0,  0, 20, 20,  0,  0,  0,
     5, -5,-10,  0,  0,-10, -5,  5,
     5, 10, 10,-20,-20, 10, 10,  5,
     0,  0,  0,  0,  0,  0,  0,  0,
];
#[rustfmt::skip]
const PST_KNIGHT: [i32; 64] = [
   -50,-40,-30,-30,-30,-30,-40,-50,
   -40,-20,  0,  0,  0,  0,-20,-40,
   -30,  0, 10, 15, 15, 10,  0,-30,
   -30,  5, 15, 20, 20, 15,  5,-30,
   -30,  0, 15, 20, 20, 15,  0,-30,
   -30,  5, 10, 15, 15, 10,  5,-30,
   -40,-20,  0,  5,  5,  0,-20,-40,
   -50,-40,-30,-30,-30,-30,-40,-50,
];
#[rustfmt::skip]
const PST_BISHOP: [i32; 64] = [
   -20,-10,-10,-10,-10,-10,-10,-20,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -10,  0,  5, 10, 10,  5,  0,-10,
   -10,  5,  5, 10, 10,  5,  5,-10,
   -10,  0, 10, 10, 10, 10,  0,-10,
   -10, 10, 10, 10, 10, 10, 10,-10,
   -10,  5,  0,  0,  0,  0,  5,-10,
   -20,-10,-10,-10,-10,-10,-10,-20,
];
#[rustfmt::skip]
const PST_ROOK: [i32; 64] = [
     0,  0,  0,  0,  0,  0,  0,  0,
     5, 10, 10, 10, 10, 10, 10,  5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
    -5,  0,  0,  0,  0,  0,  0, -5,
     0,  0,  0,  5,  5,  0,  0,  0,
];
#[rustfmt::skip]
const PST_QUEEN: [i32; 64] = [
   -20,-10,-10, -5, -5,-10,-10,-20,
   -10,  0,  0,  0,  0,  0,  0,-10,
   -10,  0,  5,  5,  5,  5,  0,-10,
    -5,  0,  5,  5,  5,  5,  0, -5,
     0,  0,  5,  5,  5,  5,  0, -5,
   -10,  5,  5,  5,  5,  5,  0,-10,
   -10,  0,  5,  0,  0,  0,  0,-10,
   -20,-10,-10, -5, -5,-10,-10,-20,
];
#[rustfmt::skip]
const PST_KING_MG: [i32; 64] = [
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -30,-40,-40,-50,-50,-40,-40,-30,
   -20,-30,-30,-40,-40,-30,-30,-20,
   -10,-20,-20,-20,-20,-20,-20,-10,
    20, 20,  0,  0,  0,  0, 20, 20,
    20, 30, 10,  0,  0, 10, 30, 20,
];
#[rustfmt::skip]
const PST_KING_EG: [i32; 64] = [
   -50,-40,-30,-20,-20,-30,-40,-50,
   -30,-20,-10,  0,  0,-10,-20,-30,
   -30,-10, 20, 30, 30, 20,-10,-30,
   -30,-10, 30, 40, 40, 30,-10,-30,
   -30,-10, 30, 40, 40, 30,-10,-30,
   -30,-10, 20, 30, 30, 20,-10,-30,
   -30,-30,  0,  0,  0,  0,-30,-30,
   -50,-30,-30,-30,-30,-30,-30,-50,
];

const PASSED_BONUS: [i32; 8] = [0, 5, 10, 20, 35, 60, 100, 0];
const PHASE_WEIGHT: [i32; 7] = [0, 0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

#[inline]
fn pst_index(color: u8, sq: u8) -> usize {
    let (f, r) = (file_of(sq) as usize, rank_of(sq) as usize);
    if color == WHITE {
        (7 - r) * 8 + f
    } else {
        r * 8 + f
    }
}

/// 0 = pure endgame, MAX_PHASE = full middlegame.
pub fn game_phase(b: &Board) -> i32 {
    b.sq.iter().map(|&p| PHASE_WEIGHT[piece_type(p) as usize]).sum::<i32>().min(MAX_PHASE)
}

#[inline]
fn taper(mg: i32, eg: i32, phase: i32) -> i32 {
    (mg * phase + eg * (MAX_PHASE - phase)) / MAX_PHASE
}

/// Full evaluation broken down by concept (White's point of view).
pub fn breakdown(b: &Board) -> Breakdown {
    let mut t = [0i32; N_CONCEPTS];
    let phase = game_phase(b);

    // Pawn files per color, for structure, passers and rook files.
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
        let idx = pst_index(c, s);

        t[Concept::Material as usize] += sign * PIECE_VALUE[kind as usize];

        match kind {
            PAWN => {
                t[Concept::PieceSquares as usize] += sign * PST_PAWN[idx];
                let f = file_of(s) as usize;
                let rel_rank = if c == WHITE { rank_of(s) } else { 7 - rank_of(s) } as usize;
                let n = pawn_files[c as usize][f] as i32;
                if n > 1 {
                    // Each extra pawn on a file costs 15; spread across the pawns there.
                    t[Concept::PawnStructure as usize] -= sign * 15 * (n - 1) / n;
                }
                let left = if f > 0 { pawn_files[c as usize][f - 1] } else { 0 };
                let right = if f < 7 { pawn_files[c as usize][f + 1] } else { 0 };
                if left == 0 && right == 0 {
                    t[Concept::PawnStructure as usize] -= sign * 12;
                }
                if is_passed(b, s, c) {
                    let bonus = PASSED_BONUS[rel_rank];
                    t[Concept::PassedPawns as usize] += sign * taper(bonus * 6 / 10, bonus * 12 / 10, phase);
                }
            }
            KNIGHT => {
                t[Concept::PieceSquares as usize] += sign * PST_KNIGHT[idx];
                let mob = knight_targets(s).iter().filter(|&&q| own_free(b, q, c)).count() as i32;
                t[Concept::Mobility as usize] += sign * 4 * (mob - 4);
            }
            BISHOP => {
                bishops[c as usize] += 1;
                t[Concept::PieceSquares as usize] += sign * PST_BISHOP[idx];
                t[Concept::Mobility as usize] += sign * 5 * (slider_mobility(b, s, c, &BISHOP_D) - 6);
            }
            ROOK => {
                t[Concept::PieceSquares as usize] += sign * PST_ROOK[idx];
                t[Concept::Mobility as usize] += sign * 2 * (slider_mobility(b, s, c, &ROOK_D) - 7);
                let f = file_of(s) as usize;
                if pawn_files[c as usize][f] == 0 {
                    t[Concept::RookFiles as usize] += sign * if pawn_files[(c ^ 1) as usize][f] == 0 { 20 } else { 10 };
                }
            }
            QUEEN => {
                t[Concept::PieceSquares as usize] += sign * PST_QUEEN[idx];
                let m = slider_mobility(b, s, c, &BISHOP_D) + slider_mobility(b, s, c, &ROOK_D);
                t[Concept::Mobility as usize] += sign * (m - 13);
            }
            KING => {
                t[Concept::PieceSquares as usize] += sign * taper(PST_KING_MG[idx], 0, phase);
                t[Concept::KingActivity as usize] += sign * taper(0, PST_KING_EG[idx], phase);
                t[Concept::KingSafety as usize] += sign * taper(king_safety(b, s, c, &pawn_files), 0, phase);
            }
            _ => {}
        }
    }

    if bishops[0] >= 2 {
        t[Concept::BishopPair as usize] += 30;
    }
    if bishops[1] >= 2 {
        t[Concept::BishopPair as usize] -= 30;
    }

    Breakdown { terms: t }
}

/// Evaluation from the side to move's point of view, all concepts at full weight.
pub fn evaluate(b: &Board) -> i32 {
    let s = breakdown(b).total();
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
        let ranks: Vec<i8> = if c == WHITE { (r + 1..8).collect() } else { (0..r).collect() };
        for rr in ranks {
            if b.sq[(rr * 8 + ff) as usize] == enemy {
                return false;
            }
        }
    }
    true
}

fn king_safety(b: &Board, k: u8, c: u8, pawn_files: &[[u8; 8]; 2]) -> i32 {
    let mut score = 0;
    let f = file_of(k);
    let r = rank_of(k);
    let fwd: i8 = if c == WHITE { 1 } else { -1 };
    let own_pawn = make_piece(c, PAWN);
    // Pawn shield: own pawns one or two ranks in front of the king.
    for df in -1..=1i8 {
        let ff = f + df;
        if !(0..8).contains(&ff) {
            continue;
        }
        for dist in 1..=2i8 {
            let rr = r + fwd * dist;
            if (0..8).contains(&rr) && b.sq[(rr * 8 + ff) as usize] == own_pawn {
                score += if dist == 1 { 12 } else { 6 };
                break;
            }
        }
        if pawn_files[c as usize][ff as usize] == 0 {
            score -= 15; // open or half-open file next to the king
        }
    }
    // Enemy pieces attacking the squares around the king.
    let mut attacked = 0;
    for &q in king_targets(k) {
        if b.attacked(q, c ^ 1) {
            attacked += 1;
        }
    }
    score -= attacked * attacked * 4;
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_is_balanced() {
        assert_eq!(evaluate(&Board::startpos()), 0);
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
