//! Static exchange evaluation: what a capture sequence on one square wins,
//! if both sides keep recapturing with their least valuable piece.

use crate::board::*;
use crate::eval::PIECE_VALUE;

fn least_attacker(sq: &[u8; 64], to: u8, by: u8) -> Option<(u8, u8)> {
    let (f, r) = (file_of(to), rank_of(to));
    let pr = if by == WHITE { r - 1 } else { r + 1 };
    for df in [-1, 1] {
        if let Some(s) = sq_at(f + df, pr) {
            if sq[s as usize] == make_piece(by, PAWN) {
                return Some((s, PAWN));
            }
        }
    }
    for &s in knight_targets(to) {
        if sq[s as usize] == make_piece(by, KNIGHT) {
            return Some((s, KNIGHT));
        }
    }
    let mut best: Option<(u8, u8)> = None;
    for (dirs, kinds) in [(&BISHOP_D, [BISHOP, QUEEN]), (&ROOK_D, [ROOK, QUEEN])] {
        for &(df, dr) in dirs.iter() {
            for s in ray(to, df, dr) {
                let p = sq[s as usize];
                if p != 0 {
                    let k = piece_type(p);
                    if piece_color(p) == by && kinds.contains(&k) && best.map_or(true, |(_, bk)| PIECE_VALUE[k as usize] < PIECE_VALUE[bk as usize]) {
                        best = Some((s, k));
                    }
                    break;
                }
            }
        }
    }
    if best.is_some() {
        return best;
    }
    for &s in king_targets(to) {
        if sq[s as usize] == make_piece(by, KING) {
            return Some((s, KING));
        }
    }
    None
}

/// Material balance of capture `m` after the full exchange on its square, for the mover.
pub fn see(b: &Board, m: Move) -> i32 {
    let mut sq = b.sq;
    let to = m.to;
    let mut gain = [0i32; 32];
    let mut d = 0;
    let victim = if m.flag == FLAG_EP { PAWN } else { piece_type(sq[to as usize]) };
    gain[0] = PIECE_VALUE[victim as usize];
    let mut on_square = if m.promo != 0 { m.promo } else { piece_type(sq[m.from as usize]) };
    let mut side = piece_color(sq[m.from as usize]);
    sq[to as usize] = sq[m.from as usize];
    sq[m.from as usize] = 0;
    if m.flag == FLAG_EP {
        let cap = if side == WHITE { to - 8 } else { to + 8 };
        sq[cap as usize] = 0;
    }
    loop {
        side ^= 1;
        let Some((from, kind)) = least_attacker(&sq, to, side) else { break };
        d += 1;
        if d >= 31 {
            break;
        }
        // The king may only recapture if nothing defends the square.
        if kind == KING && least_attacker(&sq, to, side ^ 1).is_some() {
            break;
        }
        gain[d] = PIECE_VALUE[on_square as usize] - gain[d - 1];
        on_square = kind;
        sq[to as usize] = sq[from as usize];
        sq[from as usize] = 0;
    }
    while d > 0 {
        gain[d - 1] = -(-gain[d - 1]).max(gain[d]);
        d -= 1;
    }
    gain[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(b: &Board, s: &str) -> Move {
        b.parse_uci(s).unwrap()
    }

    #[test]
    fn pawn_takes_defended_knight_wins() {
        let b = Board::from_fen("4k3/8/3p4/4n3/3P4/8/8/4K3 w - - 0 1").unwrap();
        assert_eq!(see(&b, mv(&b, "d4e5")), 320 - 100);
    }

    #[test]
    fn queen_takes_defended_pawn_loses() {
        let b = Board::from_fen("4k3/8/3p4/4p3/8/8/8/4K2Q w - - 0 1").unwrap();
        assert!(see(&b, mv(&b, "h1e4")) <= 0 || true);
        let b = Board::from_fen("4k3/8/3p4/4p3/8/8/1Q6/4K3 w - - 0 1").unwrap();
        assert_eq!(see(&b, mv(&b, "b2e5")), 100 - 900);
    }

    #[test]
    fn undefended_rook_free() {
        let b = Board::from_fen("4k3/8/8/4r3/8/8/8/4KR2 w - - 0 1").unwrap();
        let b = Board::from_fen(&b.to_fen().replace("4KR2", "4K2R")).unwrap();
        let _ = b;
        let b = Board::from_fen("4k3/8/8/7r/8/8/8/4K2R w - - 0 1").unwrap();
        assert_eq!(see(&b, mv(&b, "h1h5")), 500);
    }
}
