//! SAN, so traces read like a player would say them.

use crate::board::*;

pub fn san(b: &Board, m: Move) -> String {
    let piece = b.sq[m.from as usize];
    let kind = piece_type(piece);
    let mut s = String::new();
    if m.flag == FLAG_CASTLE {
        s.push_str(if m.to > m.from { "O-O" } else { "O-O-O" });
    } else {
        let capture = b.is_capture(m);
        if kind == PAWN {
            if capture {
                s.push((b'a' + (m.from & 7)) as char);
            }
        } else {
            s.push(['?', 'P', 'N', 'B', 'R', 'Q', 'K'][kind as usize]);
            // Disambiguate between identical pieces that can reach the same square.
            let others: Vec<Move> = b
                .legal_moves()
                .into_iter()
                .filter(|o| o.to == m.to && o.from != m.from && b.sq[o.from as usize] == piece)
                .collect();
            if !others.is_empty() {
                let same_file = others.iter().any(|o| file_of(o.from) == file_of(m.from));
                let same_rank = others.iter().any(|o| rank_of(o.from) == rank_of(m.from));
                if !same_file {
                    s.push((b'a' + (m.from & 7)) as char);
                } else if !same_rank {
                    s.push((b'1' + (m.from >> 3)) as char);
                } else {
                    s.push_str(&sq_name(m.from));
                }
            }
        }
        if capture {
            s.push('x');
        }
        s.push_str(&sq_name(m.to));
        if m.promo != 0 {
            s.push('=');
            s.push(['?', 'P', 'N', 'B', 'R', 'Q', 'K'][m.promo as usize]);
        }
    }
    let after = b.make(m);
    if after.in_check() {
        s.push(if after.legal_moves().is_empty() { '#' } else { '+' });
    }
    s
}

/// SAN for a sequence of moves starting from `b`.
pub fn san_line(b: &Board, line: &[Move]) -> String {
    let mut pos = *b;
    let mut out = vec![];
    for &m in line {
        out.push(san(&pos, m));
        pos = pos.make(m);
    }
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_san() {
        let b = Board::startpos();
        assert_eq!(san(&b, b.parse_uci("g1f3").unwrap()), "Nf3");
        assert_eq!(san(&b, b.parse_uci("e2e4").unwrap()), "e4");
        let b = Board::from_fen("6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1").unwrap();
        assert_eq!(san(&b, b.parse_uci("d1d8").unwrap()), "Rd8#");
        let b = Board::from_fen("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1").unwrap();
        assert_eq!(san(&b, b.parse_uci("e1g1").unwrap()), "O-O");
        let b = Board::from_fen("4k3/8/8/8/8/8/4K3/R6R w - - 0 1").unwrap();
        assert_eq!(san(&b, b.parse_uci("a1d1").unwrap()), "Rad1");
    }
}
