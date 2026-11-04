//! Board representation, FEN parsing, legal move generation and make-move.
//!
//! Design choice for P0: a 64-square mailbox with copy-make. It is simple,
//! easy to verify with perft, and fast enough for a human-paced engine.
//! A bitboard core can replace it later behind the same API.

use std::fmt;
use std::sync::OnceLock;

pub const WHITE: u8 = 0;
pub const BLACK: u8 = 1;

pub const PAWN: u8 = 1;
pub const KNIGHT: u8 = 2;
pub const BISHOP: u8 = 3;
pub const ROOK: u8 = 4;
pub const QUEEN: u8 = 5;
pub const KING: u8 = 6;

pub const BLACK_BIT: u8 = 8;
pub const NO_SQ: u8 = 64;

pub const WK_CASTLE: u8 = 1;
pub const WQ_CASTLE: u8 = 2;
pub const BK_CASTLE: u8 = 4;
pub const BQ_CASTLE: u8 = 8;

pub const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

#[inline]
pub fn piece_type(p: u8) -> u8 {
    p & 7
}
#[inline]
pub fn piece_color(p: u8) -> u8 {
    (p >> 3) & 1
}
#[inline]
pub fn make_piece(color: u8, kind: u8) -> u8 {
    kind | if color == BLACK { BLACK_BIT } else { 0 }
}
#[inline]
pub fn file_of(sq: u8) -> i8 {
    (sq & 7) as i8
}
#[inline]
pub fn rank_of(sq: u8) -> i8 {
    (sq >> 3) as i8
}
#[inline]
pub fn sq_at(file: i8, rank: i8) -> Option<u8> {
    if (0..8).contains(&file) && (0..8).contains(&rank) {
        Some((rank * 8 + file) as u8)
    } else {
        None
    }
}

pub fn sq_name(sq: u8) -> String {
    let f = (b'a' + (sq & 7)) as char;
    let r = (b'1' + (sq >> 3)) as char;
    format!("{f}{r}")
}

pub fn parse_sq(s: &str) -> Option<u8> {
    let b = s.as_bytes();
    if b.len() != 2 {
        return None;
    }
    let f = b[0].wrapping_sub(b'a');
    let r = b[1].wrapping_sub(b'1');
    if f < 8 && r < 8 {
        Some(r * 8 + f)
    } else {
        None
    }
}

pub const FLAG_NORMAL: u8 = 0;
pub const FLAG_DOUBLE: u8 = 1;
pub const FLAG_EP: u8 = 2;
pub const FLAG_CASTLE: u8 = 3;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Move {
    pub from: u8,
    pub to: u8,
    pub promo: u8, // piece type, 0 if none
    pub flag: u8,
}

impl Move {
    pub const NULL: Move = Move { from: 0, to: 0, promo: 0, flag: 0 };

    pub fn is_null(&self) -> bool {
        self.from == 0 && self.to == 0
    }

    pub fn uci(&self) -> String {
        if self.is_null() {
            return "0000".into();
        }
        let mut s = format!("{}{}", sq_name(self.from), sq_name(self.to));
        if self.promo != 0 {
            s.push(match self.promo {
                KNIGHT => 'n',
                BISHOP => 'b',
                ROOK => 'r',
                _ => 'q',
            });
        }
        s
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.uci())
    }
}

// ---------------------------------------------------------------- Zobrist

struct Zobrist {
    pieces: [[u64; 64]; 16],
    side: u64,
    castling: [u64; 16],
    ep_file: [u64; 8],
}

fn zobrist() -> &'static Zobrist {
    static Z: OnceLock<Zobrist> = OnceLock::new();
    Z.get_or_init(|| {
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = || {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };
        let mut pieces = [[0u64; 64]; 16];
        for p in pieces.iter_mut() {
            for s in p.iter_mut() {
                *s = next();
            }
        }
        let side = next();
        let mut castling = [0u64; 16];
        for c in castling.iter_mut() {
            *c = next();
        }
        let mut ep_file = [0u64; 8];
        for e in ep_file.iter_mut() {
            *e = next();
        }
        Zobrist { pieces, side, castling, ep_file }
    })
}

// ---------------------------------------------------------------- Board

#[derive(Clone, Copy)]
pub struct Board {
    pub sq: [u8; 64],
    pub side: u8,
    pub castling: u8,
    pub ep: u8,
    pub halfmove: u16,
    pub fullmove: u16,
    pub king: [u8; 2],
    pub hash: u64,
}

const KNIGHT_D: [(i8, i8); 8] = [(1, 2), (2, 1), (2, -1), (1, -2), (-1, -2), (-2, -1), (-2, 1), (-1, 2)];
const KING_D: [(i8, i8); 8] = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)];
pub const BISHOP_D: [(i8, i8); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
pub const ROOK_D: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

fn step_table(d: &[(i8, i8); 8]) -> [Vec<u8>; 64] {
    std::array::from_fn(|s| {
        let (f, r) = (file_of(s as u8), rank_of(s as u8));
        d.iter().filter_map(|&(df, dr)| sq_at(f + df, r + dr)).collect()
    })
}

pub fn knight_targets(sq: u8) -> &'static [u8] {
    static T: OnceLock<[Vec<u8>; 64]> = OnceLock::new();
    &T.get_or_init(|| step_table(&KNIGHT_D))[sq as usize]
}

pub fn king_targets(sq: u8) -> &'static [u8] {
    static T: OnceLock<[Vec<u8>; 64]> = OnceLock::new();
    &T.get_or_init(|| step_table(&KING_D))[sq as usize]
}

/// Squares along a ray from `sq` (exclusive) in direction (df, dr), to the edge.
pub fn ray(sq: u8, df: i8, dr: i8) -> impl Iterator<Item = u8> {
    let (mut f, mut r) = (file_of(sq), rank_of(sq));
    std::iter::from_fn(move || {
        f += df;
        r += dr;
        sq_at(f, r)
    })
}

impl Board {
    pub fn startpos() -> Board {
        Board::from_fen(START_FEN).expect("valid start FEN")
    }

    pub fn from_fen(fen: &str) -> Result<Board, String> {
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() < 4 {
            return Err(format!("bad FEN: {fen}"));
        }
        let mut b = Board {
            sq: [0; 64],
            side: WHITE,
            castling: 0,
            ep: NO_SQ,
            halfmove: 0,
            fullmove: 1,
            king: [NO_SQ; 2],
            hash: 0,
        };
        let mut rank: i8 = 7;
        let mut file: i8 = 0;
        for c in parts[0].chars() {
            match c {
                '/' => {
                    rank -= 1;
                    file = 0;
                }
                '1'..='8' => file += c as i8 - '0' as i8,
                _ => {
                    let color = if c.is_ascii_uppercase() { WHITE } else { BLACK };
                    let kind = match c.to_ascii_lowercase() {
                        'p' => PAWN,
                        'n' => KNIGHT,
                        'b' => BISHOP,
                        'r' => ROOK,
                        'q' => QUEEN,
                        'k' => KING,
                        _ => return Err(format!("bad piece '{c}' in FEN")),
                    };
                    let s = sq_at(file, rank).ok_or("FEN square out of range")?;
                    b.sq[s as usize] = make_piece(color, kind);
                    if kind == KING {
                        b.king[color as usize] = s;
                    }
                    file += 1;
                }
            }
        }
        b.side = if parts[1] == "b" { BLACK } else { WHITE };
        for c in parts[2].chars() {
            b.castling |= match c {
                'K' => WK_CASTLE,
                'Q' => WQ_CASTLE,
                'k' => BK_CASTLE,
                'q' => BQ_CASTLE,
                _ => 0,
            };
        }
        b.ep = if parts[3] == "-" { NO_SQ } else { parse_sq(parts[3]).unwrap_or(NO_SQ) };
        if parts.len() > 4 {
            b.halfmove = parts[4].parse().unwrap_or(0);
        }
        if parts.len() > 5 {
            b.fullmove = parts[5].parse().unwrap_or(1);
        }
        if b.king[0] == NO_SQ || b.king[1] == NO_SQ {
            return Err("FEN missing a king".into());
        }
        b.hash = b.compute_hash();
        Ok(b)
    }

    pub fn to_fen(&self) -> String {
        let mut s = String::new();
        for r in (0..8).rev() {
            let mut empty = 0;
            for f in 0..8 {
                let p = self.sq[r * 8 + f];
                if p == 0 {
                    empty += 1;
                    continue;
                }
                if empty > 0 {
                    s.push_str(&empty.to_string());
                    empty = 0;
                }
                let c = match piece_type(p) {
                    PAWN => 'p',
                    KNIGHT => 'n',
                    BISHOP => 'b',
                    ROOK => 'r',
                    QUEEN => 'q',
                    _ => 'k',
                };
                s.push(if piece_color(p) == WHITE { c.to_ascii_uppercase() } else { c });
            }
            if empty > 0 {
                s.push_str(&empty.to_string());
            }
            if r > 0 {
                s.push('/');
            }
        }
        s.push_str(if self.side == WHITE { " w " } else { " b " });
        if self.castling == 0 {
            s.push('-');
        }
        for (bit, c) in [(WK_CASTLE, 'K'), (WQ_CASTLE, 'Q'), (BK_CASTLE, 'k'), (BQ_CASTLE, 'q')] {
            if self.castling & bit != 0 {
                s.push(c);
            }
        }
        s.push(' ');
        s.push_str(&if self.ep == NO_SQ { "-".to_string() } else { sq_name(self.ep) });
        s.push_str(&format!(" {} {}", self.halfmove, self.fullmove));
        s
    }

    pub fn compute_hash(&self) -> u64 {
        let z = zobrist();
        let mut h = 0u64;
        for s in 0..64 {
            let p = self.sq[s];
            if p != 0 {
                h ^= z.pieces[p as usize][s];
            }
        }
        if self.side == BLACK {
            h ^= z.side;
        }
        h ^= z.castling[self.castling as usize];
        if self.ep != NO_SQ {
            h ^= z.ep_file[(self.ep & 7) as usize];
        }
        h
    }

    /// Is `sq` attacked by any piece of color `by`?
    pub fn attacked(&self, sq: u8, by: u8) -> bool {
        let (f, r) = (file_of(sq), rank_of(sq));
        // Pawns: a pawn of color `by` attacks sq from one rank "behind" it.
        let pr = if by == WHITE { r - 1 } else { r + 1 };
        for df in [-1, 1] {
            if let Some(s) = sq_at(f + df, pr) {
                if self.sq[s as usize] == make_piece(by, PAWN) {
                    return true;
                }
            }
        }
        for &s in knight_targets(sq) {
            if self.sq[s as usize] == make_piece(by, KNIGHT) {
                return true;
            }
        }
        for &s in king_targets(sq) {
            if self.sq[s as usize] == make_piece(by, KING) {
                return true;
            }
        }
        for &(df, dr) in BISHOP_D.iter() {
            for s in ray(sq, df, dr) {
                let p = self.sq[s as usize];
                if p != 0 {
                    if piece_color(p) == by && matches!(piece_type(p), BISHOP | QUEEN) {
                        return true;
                    }
                    break;
                }
            }
        }
        for &(df, dr) in ROOK_D.iter() {
            for s in ray(sq, df, dr) {
                let p = self.sq[s as usize];
                if p != 0 {
                    if piece_color(p) == by && matches!(piece_type(p), ROOK | QUEEN) {
                        return true;
                    }
                    break;
                }
            }
        }
        false
    }

    pub fn in_check(&self) -> bool {
        self.attacked(self.king[self.side as usize], self.side ^ 1)
    }

    fn push_pawn_moves(list: &mut Vec<Move>, from: u8, to: u8, flag: u8) {
        if rank_of(to) == 0 || rank_of(to) == 7 {
            for promo in [QUEEN, ROOK, BISHOP, KNIGHT] {
                list.push(Move { from, to, promo, flag });
            }
        } else {
            list.push(Move { from, to, promo: 0, flag });
        }
    }

    /// Pseudo-legal moves (may leave own king in check).
    pub fn pseudo_moves(&self, captures_only: bool) -> Vec<Move> {
        let mut list = Vec::with_capacity(48);
        let us = self.side;
        let them = us ^ 1;
        for from in 0..64u8 {
            let p = self.sq[from as usize];
            if p == 0 || piece_color(p) != us {
                continue;
            }
            let (f, r) = (file_of(from), rank_of(from));
            match piece_type(p) {
                PAWN => {
                    let dir: i8 = if us == WHITE { 1 } else { -1 };
                    let start_rank = if us == WHITE { 1 } else { 6 };
                    if let Some(to) = sq_at(f, r + dir) {
                        let promo_push = rank_of(to) == 0 || rank_of(to) == 7;
                        if self.sq[to as usize] == 0 && (!captures_only || promo_push) {
                            Self::push_pawn_moves(&mut list, from, to, FLAG_NORMAL);
                            if r == start_rank && !captures_only {
                                let to2 = sq_at(f, r + 2 * dir).unwrap();
                                if self.sq[to2 as usize] == 0 {
                                    list.push(Move { from, to: to2, promo: 0, flag: FLAG_DOUBLE });
                                }
                            }
                        }
                    }
                    for df in [-1, 1] {
                        if let Some(to) = sq_at(f + df, r + dir) {
                            let t = self.sq[to as usize];
                            if t != 0 && piece_color(t) == them {
                                Self::push_pawn_moves(&mut list, from, to, FLAG_NORMAL);
                            } else if to == self.ep {
                                list.push(Move { from, to, promo: 0, flag: FLAG_EP });
                            }
                        }
                    }
                }
                KNIGHT | KING => {
                    let targets = if piece_type(p) == KNIGHT { knight_targets(from) } else { king_targets(from) };
                    for &to in targets {
                        let t = self.sq[to as usize];
                        if t == 0 {
                            if !captures_only {
                                list.push(Move { from, to, promo: 0, flag: FLAG_NORMAL });
                            }
                        } else if piece_color(t) == them {
                            list.push(Move { from, to, promo: 0, flag: FLAG_NORMAL });
                        }
                    }
                }
                kind => {
                    let dirs: &[(i8, i8)] = match kind {
                        BISHOP => &BISHOP_D,
                        ROOK => &ROOK_D,
                        _ => &[(1, 1), (1, -1), (-1, 1), (-1, -1), (1, 0), (-1, 0), (0, 1), (0, -1)],
                    };
                    for &(df, dr) in dirs {
                        for to in ray(from, df, dr) {
                            let t = self.sq[to as usize];
                            if t == 0 {
                                if !captures_only {
                                    list.push(Move { from, to, promo: 0, flag: FLAG_NORMAL });
                                }
                            } else {
                                if piece_color(t) == them {
                                    list.push(Move { from, to, promo: 0, flag: FLAG_NORMAL });
                                }
                                break;
                            }
                        }
                    }
                }
            }
        }
        if !captures_only {
            self.castle_moves(&mut list);
        }
        list
    }

    fn castle_moves(&self, list: &mut Vec<Move>) {
        let us = self.side;
        let them = us ^ 1;
        let (base, k_right, q_right) = if us == WHITE { (0u8, WK_CASTLE, WQ_CASTLE) } else { (56u8, BK_CASTLE, BQ_CASTLE) };
        let king_sq = base + 4;
        if self.sq[king_sq as usize] != make_piece(us, KING) {
            return;
        }
        let rook = make_piece(us, ROOK);
        if self.castling & k_right != 0
            && self.sq[(base + 7) as usize] == rook
            && self.sq[(base + 5) as usize] == 0
            && self.sq[(base + 6) as usize] == 0
            && !self.attacked(king_sq, them)
            && !self.attacked(base + 5, them)
            && !self.attacked(base + 6, them)
        {
            list.push(Move { from: king_sq, to: base + 6, promo: 0, flag: FLAG_CASTLE });
        }
        if self.castling & q_right != 0
            && self.sq[base as usize] == rook
            && self.sq[(base + 1) as usize] == 0
            && self.sq[(base + 2) as usize] == 0
            && self.sq[(base + 3) as usize] == 0
            && !self.attacked(king_sq, them)
            && !self.attacked(base + 3, them)
            && !self.attacked(base + 2, them)
        {
            list.push(Move { from: king_sq, to: base + 2, promo: 0, flag: FLAG_CASTLE });
        }
    }

    /// Apply a pseudo-legal move and return the new board (copy-make).
    pub fn make(&self, m: Move) -> Board {
        self.make_inner(m, true)
    }

    /// Copy-make with an incrementally updated Zobrist hash. `with_hash` is
    /// false for the legality test, which only needs the squares.
    fn make_inner(&self, m: Move, with_hash: bool) -> Board {
        let z = zobrist();
        let mut b = *self;
        let us = self.side;
        let piece = b.sq[m.from as usize];
        let captured = b.sq[m.to as usize];
        let placed = if m.promo != 0 { make_piece(us, m.promo) } else { piece };
        let mut h = self.hash;

        b.sq[m.from as usize] = 0;
        b.sq[m.to as usize] = placed;
        if with_hash {
            h ^= z.pieces[piece as usize][m.from as usize] ^ z.pieces[placed as usize][m.to as usize];
            if captured != 0 {
                h ^= z.pieces[captured as usize][m.to as usize];
            }
        }

        match m.flag {
            FLAG_EP => {
                let cap_sq = if us == WHITE { m.to - 8 } else { m.to + 8 };
                if with_hash {
                    h ^= z.pieces[b.sq[cap_sq as usize] as usize][cap_sq as usize];
                }
                b.sq[cap_sq as usize] = 0;
            }
            FLAG_CASTLE => {
                let (rf, rt) = if m.to > m.from { (m.from + 3, m.from + 1) } else { (m.from - 4, m.from - 1) };
                let rook = b.sq[rf as usize];
                if with_hash {
                    h ^= z.pieces[rook as usize][rf as usize] ^ z.pieces[rook as usize][rt as usize];
                }
                b.sq[rt as usize] = rook;
                b.sq[rf as usize] = 0;
            }
            _ => {}
        }

        if piece_type(piece) == KING {
            b.king[us as usize] = m.to;
        }
        if !with_hash {
            b.side = us ^ 1;
            return b;
        }

        // Castling rights lost when a king or rook moves, or a rook is captured.
        for s in [m.from, m.to] {
            b.castling &= match s {
                0 => !WQ_CASTLE,
                4 => !(WK_CASTLE | WQ_CASTLE),
                7 => !WK_CASTLE,
                56 => !BQ_CASTLE,
                60 => !(BK_CASTLE | BQ_CASTLE),
                63 => !BK_CASTLE,
                _ => 0xFF,
            };
        }
        h ^= z.castling[self.castling as usize] ^ z.castling[b.castling as usize];

        if self.ep != NO_SQ {
            h ^= z.ep_file[(self.ep & 7) as usize];
        }
        b.ep = if m.flag == FLAG_DOUBLE { (m.from + m.to) / 2 } else { NO_SQ };
        if b.ep != NO_SQ {
            h ^= z.ep_file[(b.ep & 7) as usize];
        }
        b.halfmove = if piece_type(piece) == PAWN || captured != 0 || m.flag == FLAG_EP { 0 } else { self.halfmove + 1 };
        if us == BLACK {
            b.fullmove += 1;
        }
        b.side = us ^ 1;
        b.hash = h ^ z.side;
        debug_assert_eq!(b.hash, b.compute_hash());
        b
    }

    pub fn make_null(&self) -> Board {
        let z = zobrist();
        let mut b = *self;
        b.side ^= 1;
        let mut h = self.hash ^ z.side;
        if self.ep != NO_SQ {
            h ^= z.ep_file[(self.ep & 7) as usize];
        }
        b.ep = NO_SQ;
        b.hash = h;
        b
    }

    /// Fully legal moves.
    pub fn legal_moves(&self) -> Vec<Move> {
        let check = self.in_check();
        self.pseudo_moves(false).into_iter().filter(|&m| self.is_legal_fast(m, check)).collect()
    }

    pub fn legal_captures(&self) -> Vec<Move> {
        let check = self.in_check();
        self.pseudo_moves(true).into_iter().filter(|&m| self.is_legal_fast(m, check)).collect()
    }

    /// Legality with a shortcut: when not in check, a non-king, non-en-passant
    /// move can only expose its own king if it starts on a line through the
    /// king, so every other move is legal without playing it out.
    #[inline]
    fn is_legal_fast(&self, m: Move, in_check: bool) -> bool {
        let k = self.king[self.side as usize];
        if in_check || m.from == k || m.flag == FLAG_EP {
            return self.is_legal(m);
        }
        let (df, dr) = (file_of(m.from) - file_of(k), rank_of(m.from) - rank_of(k));
        if df != 0 && dr != 0 && df.abs() != dr.abs() {
            return true;
        }
        self.is_legal(m)
    }

    pub fn is_legal(&self, m: Move) -> bool {
        let nb = self.make_inner(m, false);
        !nb.attacked(nb.king[self.side as usize], nb.side)
    }

    pub fn is_capture(&self, m: Move) -> bool {
        self.sq[m.to as usize] != 0 || m.flag == FLAG_EP
    }

    pub fn captured_type(&self, m: Move) -> u8 {
        if m.flag == FLAG_EP {
            PAWN
        } else {
            piece_type(self.sq[m.to as usize])
        }
    }

    /// Parse a UCI move string against the current legal moves.
    pub fn parse_uci(&self, s: &str) -> Option<Move> {
        self.legal_moves().into_iter().find(|m| m.uci() == s)
    }

    /// Material-only draw: K vs K, K+minor vs K.
    pub fn insufficient_material(&self) -> bool {
        let mut minors = 0;
        for &p in self.sq.iter() {
            match piece_type(p) {
                0 | KING => {}
                KNIGHT | BISHOP => minors += 1,
                _ => return false,
            }
        }
        minors <= 1
    }

    pub fn perft(&self, depth: u32) -> u64 {
        if depth == 0 {
            return 1;
        }
        let moves = self.legal_moves();
        if depth == 1 {
            return moves.len() as u64;
        }
        moves.iter().map(|&m| self.make(m).perft(depth - 1)).sum()
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for r in (0..8).rev() {
            for fl in 0..8 {
                let p = self.sq[r * 8 + fl];
                let c = match piece_type(p) {
                    0 => '.',
                    PAWN => 'p',
                    KNIGHT => 'n',
                    BISHOP => 'b',
                    ROOK => 'r',
                    QUEEN => 'q',
                    _ => 'k',
                };
                let c = if p != 0 && piece_color(p) == WHITE { c.to_ascii_uppercase() } else { c };
                write!(f, "{c} ")?;
            }
            writeln!(f)?;
        }
        write!(f, "{}", self.to_fen())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perft(fen: &str, depth: u32) -> u64 {
        Board::from_fen(fen).unwrap().perft(depth)
    }

    // Reference numbers: chessprogramming.org "Perft Results".
    #[test]
    fn perft_start() {
        assert_eq!(perft(START_FEN, 1), 20);
        assert_eq!(perft(START_FEN, 2), 400);
        assert_eq!(perft(START_FEN, 3), 8_902);
        assert_eq!(perft(START_FEN, 4), 197_281);
    }

    #[test]
    fn perft_kiwipete() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        assert_eq!(perft(fen, 1), 48);
        assert_eq!(perft(fen, 2), 2_039);
        assert_eq!(perft(fen, 3), 97_862);
    }

    #[test]
    fn perft_position3() {
        let fen = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
        assert_eq!(perft(fen, 4), 43_238);
        assert_eq!(perft(fen, 5), 674_624);
    }

    #[test]
    fn perft_position4() {
        let fen = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
        assert_eq!(perft(fen, 3), 9_467);
    }

    #[test]
    fn perft_position5() {
        let fen = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";
        assert_eq!(perft(fen, 3), 62_379);
    }

    /// Walk a perft tree and check every incremental hash against a full recompute.
    #[test]
    fn incremental_hash_matches() {
        fn walk(b: &Board, d: u32) {
            assert_eq!(b.hash, b.compute_hash(), "hash drift at {}", b.to_fen());
            assert_eq!(b.make_null().hash, b.make_null().compute_hash());
            if d == 0 {
                return;
            }
            for m in b.legal_moves() {
                walk(&b.make(m), d - 1);
            }
        }
        for fen in [
            START_FEN,
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        ] {
            walk(&Board::from_fen(fen).unwrap(), 3);
        }
    }

    #[test]
    fn fen_roundtrip() {
        let fen = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
        assert_eq!(Board::from_fen(fen).unwrap().to_fen(), fen);
    }
}
