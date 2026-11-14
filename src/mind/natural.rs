//! How natural a move looks to players of a rating. Conditional logit over simple
//! move features, weights drift linearly with elo, fit by harness/natural/fit.py.

use super::perception::{attacks_from, en_prise};
use crate::board::*;
use crate::eval::{game_phase, PIECE_VALUE};

pub const NF: usize = 34;

include!("natural_params.rs");

fn centre_dist(sq: u8) -> f32 {
    let f = file_of(sq) as f32 - 3.5;
    let r = rank_of(sq) as f32 - 3.5;
    f.abs().max(r.abs())
}

/// Features of move `m` in `b`; `last_to` is the square the opponent's last move went to.
pub fn features(b: &Board, m: Move, last_to: Option<u8>) -> [f32; NF] {
    let mut x = [0.0f32; NF];
    let p = b.sq[m.from as usize];
    let kind = piece_type(p);
    let c = piece_color(p);
    let rel = |sq: u8| if c == WHITE { rank_of(sq) } else { 7 - rank_of(sq) };
    let phase = game_phase(b);
    let opening = b.fullmove <= 12;
    let after = b.make(m);
    let capture = b.is_capture(m);
    x[(kind - 1) as usize] = 1.0;
    if capture {
        x[6] = 1.0;
        let gain = PIECE_VALUE[b.captured_type(m) as usize] - PIECE_VALUE[kind as usize];
        x[7] = (gain as f32 / 300.0).clamp(-3.0, 3.0);
        if last_to == Some(m.to) {
            x[8] = 1.0;
        }
    }
    if after.in_check() {
        x[9] = 1.0;
    }
    if m.promo != 0 {
        x[10] = 1.0;
    }
    if m.flag == FLAG_CASTLE {
        x[11] = 1.0;
    }
    if matches!(kind, KNIGHT | BISHOP) && rel(m.from) == 0 {
        x[12] = 1.0;
        if opening {
            x[13] = 1.0;
        }
    }
    if kind == QUEEN && b.fullmove <= 10 {
        x[14] = 1.0;
    }
    if kind == KING && m.flag != FLAG_CASTLE {
        if phase >= 12 {
            x[15] = 1.0;
        } else {
            x[16] = 1.0;
            x[17] = (centre_dist(m.from) - centre_dist(m.to)) / 3.0;
        }
    }
    if rel(m.to) < rel(m.from) {
        x[18] = 1.0;
    }
    let df = (file_of(m.to) - file_of(m.from)).abs();
    let dr = (rank_of(m.to) - rank_of(m.from)).abs();
    x[19] = df.max(dr) as f32 / 7.0;
    if matches!(kind, KNIGHT | BISHOP | QUEEN) {
        x[20] = (centre_dist(m.from) - centre_dist(m.to)) / 3.0;
    }
    if kind == PAWN {
        let f = file_of(m.from);
        if f == 3 || f == 4 {
            x[21] = 1.0;
        }
        if (f <= 1 || f >= 6) && phase >= 12 {
            x[22] = 1.0;
        }
        x[23] = rel(m.to) as f32 / 7.0;
        if m.flag == FLAG_DOUBLE {
            x[24] = 1.0;
        }
        // Passed pawn: no enemy pawn ahead on this or adjacent files.
        let enemy = make_piece(c ^ 1, PAWN);
        let passed = !(0..64u8).any(|s| {
            b.sq[s as usize] == enemy && (file_of(s) - file_of(m.to)).abs() <= 1 && rel(s) > rel(m.to)
        });
        if passed {
            x[25] = 1.0;
        }
        // Pawn moves in front of one's own castled king.
        let k = b.king[c as usize];
        if phase >= 12 && rel(k) == 0 && (file_of(k) - f).abs() <= 1 && (file_of(k) <= 2 || file_of(k) >= 5) {
            x[26] = 1.0;
        }
    }
    if let Some(v) = en_prise(&after, m.to) {
        x[27] = v as f32 / 300.0;
    }
    if let Some(v) = en_prise(b, m.from) {
        x[28] = v as f32 / 300.0;
    }
    if kind == KNIGHT && (file_of(m.to) == 0 || file_of(m.to) == 7 || rank_of(m.to) == 0 || rank_of(m.to) == 7) {
        x[29] = 1.0;
    }
    if kind == ROOK {
        let own = make_piece(c, PAWN);
        if !(0..8).any(|r| sq_at(file_of(m.to), r).map(|s| b.sq[s as usize] == own).unwrap_or(false)) {
            x[30] = 1.0;
        }
    }
    if matches!(kind, ROOK | QUEEN) && rel(m.to) == 6 {
        x[31] = 1.0;
    }
    // Creates a threat on a more valuable enemy piece.
    if kind != KING {
        let mine = PIECE_VALUE[kind as usize];
        if attacks_from(&after, m.to, kind, c).iter().any(|&s| {
            let q = after.sq[s as usize];
            q != 0 && piece_color(q) != c && piece_type(q) != KING && PIECE_VALUE[piece_type(q) as usize] > mine
        }) {
            x[32] = 1.0;
        }
    }
    if matches!(kind, BISHOP | ROOK | QUEEN) && df.max(dr) >= 4 {
        x[33] = 1.0;
    }
    x
}

/// Log-probabilities (natural log) of each move being the one a player of `elo` reaches for.
pub fn log_probs(b: &Board, moves: &[Move], last_to: Option<u8>, elo: f32) -> Vec<f32> {
    let z = (elo - 1500.0) / 500.0;
    let logits: Vec<f32> = moves
        .iter()
        .map(|&m| {
            let x = features(b, m, last_to);
            // no check term: the mind already likes checks, doubling it gave way too many
            (0..NF).filter(|&i| i != 9).map(|i| x[i] * (NAT_W[i] + z * NAT_V[i])).sum()
        })
        .collect();
    let mx = logits.iter().cloned().fold(f32::MIN, f32::max);
    let lse = mx + logits.iter().map(|l| (l - mx).exp()).sum::<f32>().ln();
    logits.iter().map(|l| l - lse).collect()
}
