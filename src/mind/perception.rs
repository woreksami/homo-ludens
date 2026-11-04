//! Stages 1-2: perception and pattern recognition.
//!
//! A human does not "generate all legal moves". Moves come to mind because
//! something on the board is salient: a capture, a check, a threatened piece,
//! a familiar pattern. Some legal moves are simply never noticed. This module
//! decides, for a given persona, which moves enter awareness and why.
//!
//! The pattern recognisers here are the hand-written v0 chunk library.
//! In P1 the learned chunk/template library plugs in through `Pattern`.

use super::persona::{Persona, Rng};
use crate::board::*;
use crate::eval::PIECE_VALUE;

/// Why a move came to mind. Reasons feed both intuition and the thought trace.
#[derive(Clone, Debug, PartialEq)]
pub enum Reason {
    Capture { value: i32 },
    Check,
    Mate,
    SavesThreatenedPiece { value: i32 },
    Pattern { name: &'static str, bonus: i32 },
    Development,
    Castle,
    Promotion,
    Quiet,
}

impl Reason {
    pub fn describe(&self) -> String {
        match self {
            Reason::Capture { value } => format!("wins material ({value})"),
            Reason::Check => "gives check".into(),
            Reason::Mate => "checkmate".into(),
            Reason::SavesThreatenedPiece { value } => format!("rescues a threatened piece ({value})"),
            Reason::Pattern { name, .. } => format!("pattern: {name}"),
            Reason::Development => "develops a piece".into(),
            Reason::Castle => "castles to safety".into(),
            Reason::Promotion => "promotes".into(),
            Reason::Quiet => "quiet move".into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Percept {
    pub mv: Move,
    pub salience: f32,
    pub reasons: Vec<Reason>,
}

/// Value of the least valuable attacker of `sq` belonging to `by`, if any.
pub fn least_attacker_value(b: &Board, sq: u8, by: u8) -> Option<i32> {
    let mut best: Option<i32> = None;
    let mut consider = |v: i32| best = Some(best.map_or(v, |x: i32| x.min(v)));
    let (f, r) = (file_of(sq), rank_of(sq));
    let pr = if by == WHITE { r - 1 } else { r + 1 };
    for df in [-1i8, 1] {
        let ff = f + df;
        if (0..8).contains(&ff) && (0..8).contains(&pr) && b.sq[(pr * 8 + ff) as usize] == make_piece(by, PAWN) {
            consider(PIECE_VALUE[PAWN as usize]);
        }
    }
    for &s in knight_targets(sq) {
        if b.sq[s as usize] == make_piece(by, KNIGHT) {
            consider(PIECE_VALUE[KNIGHT as usize]);
        }
    }
    for (dirs, kinds) in [(&BISHOP_D, [BISHOP, QUEEN]), (&ROOK_D, [ROOK, QUEEN])] {
        for &(df, dr) in dirs.iter() {
            for s in ray(sq, df, dr) {
                let p = b.sq[s as usize];
                if p != 0 {
                    if piece_color(p) == by && kinds.contains(&piece_type(p)) {
                        consider(PIECE_VALUE[piece_type(p) as usize]);
                    }
                    break;
                }
            }
        }
    }
    for &s in king_targets(sq) {
        if b.sq[s as usize] == make_piece(by, KING) {
            consider(2000);
        }
    }
    best
}

/// Is the piece on `sq` en prise: attacked and either undefended or attacked by
/// something cheaper? Returns the material a capture would net, roughly.
pub fn en_prise(b: &Board, sq: u8) -> Option<i32> {
    let p = b.sq[sq as usize];
    if p == 0 || piece_type(p) == KING {
        return None;
    }
    let owner = piece_color(p);
    let value = PIECE_VALUE[piece_type(p) as usize];
    let attacker = least_attacker_value(b, sq, owner ^ 1)?;
    let defended = b.attacked(sq, owner);
    if !defended {
        Some(value)
    } else if attacker < value {
        Some(value - attacker)
    } else {
        None
    }
}

fn is_backward(b: &Board, m: Move) -> bool {
    let c = piece_color(b.sq[m.from as usize]);
    if c == WHITE {
        rank_of(m.to) < rank_of(m.from)
    } else {
        rank_of(m.to) > rank_of(m.from)
    }
}

fn is_long(m: Move) -> bool {
    let df = (file_of(m.to) - file_of(m.from)).abs();
    let dr = (rank_of(m.to) - rank_of(m.from)).abs();
    df.max(dr) >= 4
}

/// The squares attacked by a piece of `kind` and color `c` standing on `s`.
pub fn attacks_from(b: &Board, s: u8, kind: u8, c: u8) -> Vec<u8> {
    match kind {
        PAWN => {
            let dr = if c == WHITE { 1 } else { -1 };
            [-1i8, 1]
                .iter()
                .filter_map(|&df| {
                    let (f, r) = (file_of(s) + df, rank_of(s) + dr);
                    ((0..8).contains(&f) && (0..8).contains(&r)).then(|| (r * 8 + f) as u8)
                })
                .collect()
        }
        KNIGHT => knight_targets(s).to_vec(),
        KING => king_targets(s).to_vec(),
        _ => {
            let dirs: Vec<(i8, i8)> = match kind {
                BISHOP => BISHOP_D.to_vec(),
                ROOK => ROOK_D.to_vec(),
                _ => BISHOP_D.iter().chain(ROOK_D.iter()).copied().collect(),
            };
            let mut out = vec![];
            for (df, dr) in dirs {
                for q in ray(s, df, dr) {
                    out.push(q);
                    if b.sq[q as usize] != 0 {
                        break;
                    }
                }
            }
            out
        }
    }
}

// ------------------------------------------------------------ v0 chunk library

/// A recognisable chess pattern. `elo` is the rating at which half of players
/// reliably recognise it (a prior, to be fitted in P1).
pub struct PatternDef {
    pub name: &'static str,
    pub elo: f32,
    pub detect: fn(&Board, Move, &Board) -> Option<i32>,
}

/// Fork: the moved piece attacks two or more enemy pieces worth more than it,
/// or the king plus another valuable piece, from a square where it is safe.
fn detect_fork(b: &Board, m: Move, after: &Board) -> Option<i32> {
    let us = b.side;
    let piece = after.sq[m.to as usize];
    let kind = piece_type(piece);
    if en_prise(after, m.to).is_some() {
        return None;
    }
    let my_value = PIECE_VALUE[kind as usize];
    let mut hits = vec![];
    for q in attacks_from(after, m.to, kind, us) {
        let t = after.sq[q as usize];
        if t != 0 && piece_color(t) != us {
            let tk = piece_type(t);
            if tk == KING {
                hits.push(1000);
            } else if PIECE_VALUE[tk as usize] > my_value || !after.attacked(q, us ^ 1) {
                hits.push(PIECE_VALUE[tk as usize]);
            }
        }
    }
    if hits.len() >= 2 {
        hits.sort_unstable();
        Some(hits[hits.len() - 2].min(500) / 2)
    } else {
        None
    }
}

/// Pin or skewer: a slider lines up an enemy piece with a more valuable piece
/// (or the king) behind it.
fn detect_pin_skewer(b: &Board, m: Move, after: &Board) -> Option<i32> {
    let us = b.side;
    let kind = piece_type(after.sq[m.to as usize]);
    let dirs: Vec<(i8, i8)> = match kind {
        BISHOP => BISHOP_D.to_vec(),
        ROOK => ROOK_D.to_vec(),
        QUEEN => BISHOP_D.iter().chain(ROOK_D.iter()).copied().collect(),
        _ => return None,
    };
    if en_prise(after, m.to).is_some() {
        return None;
    }
    for (df, dr) in dirs {
        let mut seen: Vec<u8> = vec![];
        for q in ray(m.to, df, dr) {
            let t = after.sq[q as usize];
            if t == 0 {
                continue;
            }
            if piece_color(t) == us {
                break;
            }
            seen.push(piece_type(t));
            if seen.len() == 2 {
                let front = PIECE_VALUE[seen[0] as usize];
                let back = if seen[1] == KING { 2000 } else { PIECE_VALUE[seen[1] as usize] };
                if back > front.max(PIECE_VALUE[kind as usize]) || (seen[0] == KING) {
                    return Some(60);
                }
                break;
            }
        }
    }
    None
}

/// Back-rank mate idea: a heavy piece checks on the enemy back rank.
fn detect_back_rank(b: &Board, m: Move, after: &Board) -> Option<i32> {
    let kind = piece_type(after.sq[m.to as usize]);
    if !matches!(kind, ROOK | QUEEN) || !after.in_check() {
        return None;
    }
    let back = if b.side == WHITE { 7 } else { 0 };
    (rank_of(m.to) == back && rank_of(after.king[after.side as usize]) == back).then_some(80)
}

pub const PATTERNS: [PatternDef; 3] = [
    PatternDef { name: "fork", elo: 1000.0, detect: detect_fork },
    PatternDef { name: "pin or skewer", elo: 1350.0, detect: detect_pin_skewer },
    PatternDef { name: "back-rank attack", elo: 900.0, detect: detect_back_rank },
];

// ------------------------------------------------------------ perception

/// Pieces of the side to move that the opponent threatens, and their value at risk.
pub fn threatened_pieces(b: &Board) -> Vec<(u8, i32)> {
    (0..64u8)
        .filter(|&s| {
            let p = b.sq[s as usize];
            p != 0 && piece_color(p) == b.side
        })
        .filter_map(|s| en_prise(b, s).map(|v| (s, v)))
        .collect()
}

/// Which legal moves come to this player's mind, with why and how salient.
/// `rng` makes perception stochastic but reproducible.
pub fn perceive(b: &Board, persona: &Persona, rng: &mut Rng) -> Vec<Percept> {
    let legal = b.legal_moves();
    let threats = threatened_pieces(b);
    let noticed_threats: Vec<(u8, i32)> = threats.into_iter().filter(|_| rng.chance(persona.p_see_threat)).collect();
    let opening = b.fullmove <= 12;
    let mut out = Vec::with_capacity(legal.len());

    for m in legal {
        let after = b.make(m);
        let piece = b.sq[m.from as usize];
        let kind = piece_type(piece);
        let mut reasons = vec![];
        let mut salience: f32 = 0.0;

        if after.legal_moves().is_empty() && after.in_check() {
            reasons.push(Reason::Mate);
            salience += 3.0;
        } else if after.in_check() {
            reasons.push(Reason::Check);
            salience += 1.2;
        }
        if b.is_capture(m) {
            let v = PIECE_VALUE[b.captured_type(m) as usize];
            reasons.push(Reason::Capture { value: v });
            salience += 1.0 + v as f32 / 300.0;
        }
        if m.promo != 0 {
            reasons.push(Reason::Promotion);
            salience += 2.0;
        }
        if m.flag == FLAG_CASTLE {
            reasons.push(Reason::Castle);
            salience += 0.8;
        }
        if let Some(&(_, v)) = noticed_threats.iter().find(|&&(s, _)| s == m.from) {
            reasons.push(Reason::SavesThreatenedPiece { value: v });
            salience += 1.0 + v as f32 / 300.0;
        }
        for pat in PATTERNS.iter() {
            if let Some(bonus) = (pat.detect)(b, m, &after) {
                if rng.chance(persona.recognises(pat.elo)) {
                    reasons.push(Reason::Pattern { name: pat.name, bonus });
                    salience += 1.0;
                }
            }
        }
        if opening && matches!(kind, KNIGHT | BISHOP) {
            let home = if piece_color(piece) == WHITE { 0 } else { 7 };
            if rank_of(m.from) == home {
                reasons.push(Reason::Development);
                salience += 0.6;
            }
        }
        if reasons.is_empty() {
            reasons.push(Reason::Quiet);
        }

        // Does this move enter awareness at all?
        let forcing = salience >= 1.0;
        let mut p_notice = if forcing {
            if b.is_capture(m) {
                persona.p_see_capture
            } else {
                0.97
            }
        } else {
            // Quiet moves: weaker players notice fewer of them.
            0.35 + 0.6 * persona.p_see_threat
        };
        if is_backward(b, m) {
            p_notice *= 1.0 - persona.p_blind_backward;
        }
        if matches!(kind, BISHOP | ROOK | QUEEN) && is_long(m) {
            p_notice *= 1.0 - persona.p_blind_long;
        }
        if reasons.contains(&Reason::Mate) {
            p_notice = p_notice.max(persona.p_see_capture);
        }
        if rng.chance(p_notice) {
            out.push(Percept { mv: m, salience, reasons });
        }
    }

    // A player always sees at least one move.
    if out.is_empty() {
        let legal = b.legal_moves();
        if !legal.is_empty() {
            let m = legal[(rng.next_u64() % legal.len() as u64) as usize];
            out.push(Percept { mv: m, salience: 0.0, reasons: vec![Reason::Quiet] });
        }
    }
    out
}

/// The opponent replies this player considers while calculating: forcing
/// replies first, quiet ones only sometimes, capped at `persona.opp_replies`.
pub fn perceive_replies(b: &Board, persona: &Persona, rng: &mut Rng, forgotten_from: &[u8]) -> Vec<Move> {
    let mut scored: Vec<(f32, Move)> = vec![];
    for m in b.legal_moves() {
        // Working-memory lapse: a piece that moved earlier in this line is not
        // where the player thinks it is, so its moves are not imagined.
        if forgotten_from.contains(&m.from) {
            continue;
        }
        let after = b.make(m);
        let mut s = 0.0;
        if b.is_capture(m) {
            s += 2.0 + PIECE_VALUE[b.captured_type(m) as usize] as f32 / 300.0;
        }
        if after.in_check() {
            s += 1.8;
        }
        if m.promo != 0 {
            s += 2.0;
        }
        if s == 0.0 {
            if rng.chance(persona.p_blind_quiet) {
                continue;
            }
            s = 0.3;
        }
        if is_backward(b, m) && rng.chance(persona.p_blind_backward) {
            continue;
        }
        scored.push((s, m));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    scored.truncate(persona.opp_replies.max(1));
    scored.into_iter().map(|(_, m)| m).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sees_hanging_piece() {
        let b = Board::from_fen("4k3/8/8/3q4/8/8/3R4/4K3 w - - 0 1").unwrap();
        assert_eq!(en_prise(&b, parse_sq("d5").unwrap()), Some(900));
    }

    #[test]
    fn detects_knight_fork() {
        // Nc7+ forks king e8 and rook a8.
        let b = Board::from_fen("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1").unwrap();
        let m = b.parse_uci("b5c7").unwrap();
        assert!(detect_fork(&b, m, &b.make(m)).is_some());
    }

    #[test]
    fn finds_threatened_rook() {
        // Bishop g5 attacks the rook on d2; the king defends it, but B (330) < R (500).
        let b = Board::from_fen("4k3/8/8/6b1/8/8/3R4/4K3 w - - 0 1").unwrap();
        let t = threatened_pieces(&b);
        assert_eq!(t, vec![(parse_sq("d2").unwrap(), 170)]);
    }
}
