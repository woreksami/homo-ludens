//! Classical alpha-beta search: the engine's verification layer (stage 6).
//!
//! Iterative deepening, principal variation search, transposition table,
//! null-move pruning, late-move reductions, killer and history ordering,
//! check extension and quiescence. At full strength this is a conventional
//! engine; the mind layers decide how much of it a persona may use.

use crate::board::*;
use crate::eval::{self, PIECE_VALUE};
use std::time::Instant;

pub const INF: i32 = 32_000;
pub const MATE: i32 = 30_000;
const MAX_PLY: usize = 128;

pub fn is_mate_score(s: i32) -> bool {
    s.abs() > MATE - 1000
}

#[derive(Clone, Default, Debug)]
pub struct Limits {
    pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub movetime_ms: Option<u64>,
    /// Restrict the root to these moves (UCI `searchmoves`, or mind candidates).
    pub root_moves: Option<Vec<Move>>,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub best: Move,
    pub score: i32,
    pub depth: u32,
    pub nodes: u64,
    pub pv: Vec<Move>,
    pub elapsed_ms: u64,
}

#[derive(Clone, Copy, Default)]
struct TTEntry {
    key: u64,
    depth: i8,
    score: i32,
    flag: u8, // 0 exact, 1 lower bound, 2 upper bound
    best: Move,
}

const TT_EXACT: u8 = 0;
const TT_LOWER: u8 = 1;
const TT_UPPER: u8 = 2;

pub struct Searcher {
    tt: Vec<TTEntry>,
    killers: [[Move; 2]; MAX_PLY],
    history: Box<[[i32; 64]; 64]>,
    pub nodes: u64,
    start: Instant,
    limits: Limits,
    stopped: bool,
    /// Hashes of positions earlier in the game and along the current line.
    rep: Vec<u64>,
    pub silent: bool,
}

impl Searcher {
    pub fn new(tt_mb: usize) -> Searcher {
        let n = (tt_mb * 1024 * 1024 / std::mem::size_of::<TTEntry>()).next_power_of_two() / 2;
        Searcher {
            tt: vec![TTEntry::default(); n.max(1024)],
            killers: [[Move::NULL; 2]; MAX_PLY],
            history: Box::new([[0; 64]; 64]),
            nodes: 0,
            start: Instant::now(),
            limits: Limits::default(),
            stopped: false,
            rep: Vec::new(),
            silent: false,
        }
    }

    pub fn clear(&mut self) {
        for e in self.tt.iter_mut() {
            *e = TTEntry::default();
        }
        self.history = Box::new([[0; 64]; 64]);
    }

    /// Positions that occurred before the root (for repetition detection).
    pub fn set_game_history(&mut self, hashes: Vec<u64>) {
        self.rep = hashes;
    }

    fn tt_probe(&self, key: u64) -> Option<TTEntry> {
        let e = self.tt[(key as usize) & (self.tt.len() - 1)];
        (e.key == key).then_some(e)
    }

    fn tt_store(&mut self, key: u64, depth: i32, score: i32, flag: u8, best: Move) {
        let i = (key as usize) & (self.tt.len() - 1);
        let e = &mut self.tt[i];
        if e.key != key || depth as i8 >= e.depth || flag == TT_EXACT {
            *e = TTEntry { key, depth: depth as i8, score, flag, best };
        }
    }

    fn time_up(&mut self) -> bool {
        if self.stopped {
            return true;
        }
        if self.nodes & 1023 == 0 {
            if let Some(n) = self.limits.nodes {
                if self.nodes >= n {
                    self.stopped = true;
                }
            }
            if let Some(ms) = self.limits.movetime_ms {
                if self.start.elapsed().as_millis() as u64 >= ms {
                    self.stopped = true;
                }
            }
        }
        self.stopped
    }

    fn is_repetition(&self, b: &Board) -> bool {
        let n = self.rep.len();
        let lookback = (b.halfmove as usize).min(n);
        // Same side to move only: step back two plies at a time.
        let mut i = 2;
        while i <= lookback {
            if self.rep[n - i] == b.hash {
                return true;
            }
            i += 2;
        }
        false
    }

    fn order_score(&self, b: &Board, m: Move, tt_move: Move, ply: usize) -> i32 {
        if m == tt_move {
            return 1_000_000;
        }
        if b.is_capture(m) {
            let victim = PIECE_VALUE[b.captured_type(m) as usize];
            let attacker = PIECE_VALUE[piece_type(b.sq[m.from as usize]) as usize];
            return 100_000 + victim * 10 - attacker / 10;
        }
        if m.promo == QUEEN {
            return 90_000;
        }
        if ply < MAX_PLY {
            if m == self.killers[ply][0] {
                return 80_000;
            }
            if m == self.killers[ply][1] {
                return 79_000;
            }
        }
        self.history[m.from as usize][m.to as usize]
    }

    fn sort_moves(&self, b: &Board, moves: &mut [Move], tt_move: Move, ply: usize) {
        moves.sort_by_cached_key(|&m| -self.order_score(b, m, tt_move, ply));
    }

    pub fn quiesce(&mut self, b: &Board, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.nodes += 1;
        if self.time_up() {
            return 0;
        }
        let in_check = b.in_check();
        let stand = if in_check { -MATE + ply as i32 } else { eval::evaluate(b) };
        if !in_check {
            if stand >= beta {
                return stand;
            }
            if stand > alpha {
                alpha = stand;
            }
        }
        if ply >= MAX_PLY - 1 {
            return stand;
        }
        let mut moves = if in_check { b.legal_moves() } else { b.legal_captures() };
        if in_check && moves.is_empty() {
            return -MATE + ply as i32;
        }
        self.sort_moves(b, &mut moves, Move::NULL, MAX_PLY);
        let mut best = stand;
        for m in moves {
            // Delta pruning: a capture that cannot raise alpha even with a margin.
            if !in_check && m.promo == 0 {
                let gain = PIECE_VALUE[b.captured_type(m) as usize];
                if stand + gain + 200 < alpha {
                    continue;
                }
            }
            let score = -self.quiesce(&b.make(m), -beta, -alpha, ply + 1);
            if score > best {
                best = score;
            }
            if score > alpha {
                alpha = score;
                if alpha >= beta {
                    break;
                }
            }
        }
        best
    }

    fn negamax(&mut self, b: &Board, mut depth: i32, mut alpha: i32, beta: i32, ply: usize, allow_null: bool) -> i32 {
        if ply > 0 && (b.halfmove >= 100 || self.is_repetition(b) || b.insufficient_material()) {
            return 0;
        }
        let in_check = b.in_check();
        if in_check {
            depth += 1; // check extension
        }
        if depth <= 0 || ply >= MAX_PLY - 1 {
            return self.quiesce(b, alpha, beta, ply);
        }
        self.nodes += 1;
        if self.time_up() {
            return 0;
        }

        let pv_node = beta - alpha > 1;
        let mut tt_move = Move::NULL;
        if let Some(e) = self.tt_probe(b.hash) {
            tt_move = e.best;
            if !pv_node && e.depth as i32 >= depth && ply > 0 {
                let s = score_from_tt(e.score, ply);
                match e.flag {
                    TT_EXACT => return s,
                    TT_LOWER if s >= beta => return s,
                    TT_UPPER if s <= alpha => return s,
                    _ => {}
                }
            }
        }

        // Null-move pruning (not in check, not in pawn-only endings).
        if allow_null && !pv_node && !in_check && depth >= 3 && has_non_pawn_material(b) {
            if eval::evaluate(b) >= beta {
                let r = 2 + depth / 4;
                self.rep.push(b.hash);
                let s = -self.negamax(&b.make_null(), depth - 1 - r, -beta, -beta + 1, ply + 1, false);
                self.rep.pop();
                if s >= beta && !is_mate_score(s) {
                    return beta;
                }
            }
        }

        let mut moves = b.legal_moves();
        if moves.is_empty() {
            return if in_check { -MATE + ply as i32 } else { 0 };
        }
        self.sort_moves(b, &mut moves, tt_move, ply);

        let orig_alpha = alpha;
        let mut best = -INF;
        let mut best_move = moves[0];
        self.rep.push(b.hash);
        for (i, &m) in moves.iter().enumerate() {
            let child = b.make(m);
            let quiet = !b.is_capture(m) && m.promo == 0;
            let gives_check = child.in_check();
            let mut score;
            if i == 0 {
                score = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, true);
            } else {
                // Late-move reduction for quiet, late, non-checking moves.
                let mut r = 0;
                if depth >= 3 && i >= 3 && quiet && !in_check && !gives_check {
                    r = 1 + (i as i32 / 8).min(2);
                }
                score = -self.negamax(&child, depth - 1 - r, -alpha - 1, -alpha, ply + 1, true);
                if score > alpha && r > 0 {
                    score = -self.negamax(&child, depth - 1, -alpha - 1, -alpha, ply + 1, true);
                }
                if score > alpha && score < beta {
                    score = -self.negamax(&child, depth - 1, -beta, -alpha, ply + 1, true);
                }
            }
            if self.stopped {
                self.rep.pop();
                return 0;
            }
            if score > best {
                best = score;
                best_move = m;
            }
            if score > alpha {
                alpha = score;
                if alpha >= beta {
                    if quiet && ply < MAX_PLY {
                        if self.killers[ply][0] != m {
                            self.killers[ply][1] = self.killers[ply][0];
                            self.killers[ply][0] = m;
                        }
                        let h = &mut self.history[m.from as usize][m.to as usize];
                        *h = (*h + depth * depth).min(50_000);
                    }
                    break;
                }
            }
        }
        self.rep.pop();

        let flag = if best >= beta {
            TT_LOWER
        } else if best > orig_alpha {
            TT_EXACT
        } else {
            TT_UPPER
        };
        self.tt_store(b.hash, depth, score_to_tt(best, ply), flag, best_move);
        best
    }

    fn extract_pv(&self, b: &Board, max: usize) -> Vec<Move> {
        let mut pv = Vec::new();
        let mut pos = *b;
        let mut seen = Vec::new();
        while pv.len() < max {
            let Some(e) = self.tt_probe(pos.hash) else { break };
            if e.best.is_null() || !pos.legal_moves().contains(&e.best) || seen.contains(&pos.hash) {
                break;
            }
            seen.push(pos.hash);
            pv.push(e.best);
            pos = pos.make(e.best);
        }
        pv
    }

    /// Iterative-deepening search from the root.
    pub fn search(&mut self, b: &Board, limits: Limits) -> SearchResult {
        self.start = Instant::now();
        self.nodes = 0;
        self.stopped = false;
        self.killers = [[Move::NULL; 2]; MAX_PLY];
        self.limits = limits.clone();
        let max_depth = limits.depth.unwrap_or(64).min(64);

        let mut root = b.legal_moves();
        if let Some(rm) = &limits.root_moves {
            root.retain(|m| rm.contains(m));
        }
        let mut result = SearchResult {
            best: root.first().copied().unwrap_or(Move::NULL),
            score: 0,
            depth: 0,
            nodes: 0,
            pv: vec![],
            elapsed_ms: 0,
        };
        if root.is_empty() {
            result.score = if b.in_check() { -MATE } else { 0 };
            return result;
        }

        let mut scores: Vec<i32> = vec![0; root.len()];
        for depth in 1..=max_depth as i32 {
            // Order root moves by the previous iteration's scores.
            let mut order: Vec<usize> = (0..root.len()).collect();
            order.sort_by_key(|&i| -scores[i]);
            let ordered: Vec<Move> = order.iter().map(|&i| root[i]).collect();
            let prev: Vec<i32> = order.iter().map(|&i| scores[i]).collect();
            root = ordered;
            scores = prev;

            let mut alpha = -INF;
            let beta = INF;
            let mut best_i = 0;
            self.rep.push(b.hash);
            for (i, &m) in root.iter().enumerate() {
                let child = b.make(m);
                let mut s;
                if i == 0 {
                    s = -self.negamax(&child, depth - 1, -beta, -alpha, 1, true);
                } else {
                    s = -self.negamax(&child, depth - 1, -alpha - 1, -alpha, 1, true);
                    if s > alpha && !self.stopped {
                        s = -self.negamax(&child, depth - 1, -beta, -alpha, 1, true);
                    }
                }
                if self.stopped {
                    break;
                }
                scores[i] = s;
                if s > alpha {
                    alpha = s;
                    best_i = i;
                }
            }
            self.rep.pop();
            if self.stopped && depth > 1 {
                break;
            }
            // Keep the TT pointing at the root's best move for PV extraction.
            self.tt_store(b.hash, depth, score_to_tt(alpha, 0), TT_EXACT, root[best_i]);
            let mut pv = vec![root[best_i]];
            pv.extend(self.extract_pv(&b.make(root[best_i]), depth as usize + 4));
            result = SearchResult {
                best: root[best_i],
                score: alpha,
                depth: depth as u32,
                nodes: self.nodes,
                pv,
                elapsed_ms: self.start.elapsed().as_millis() as u64,
            };
            scores[best_i] = alpha;
            if !self.silent {
                println!("{}", info_line(&result));
            }
            if is_mate_score(alpha) && depth as i32 > (MATE - alpha.abs()) + 2 {
                break;
            }
            if self.stopped {
                break;
            }
        }
        result.nodes = self.nodes;
        result.elapsed_ms = self.start.elapsed().as_millis() as u64;
        result
    }

    /// Exact score of each given root move at a fixed depth (side to move's view).
    /// Used by the mind to check its own candidates.
    pub fn score_moves(&mut self, b: &Board, moves: &[Move], depth: u32, node_budget: Option<u64>) -> Vec<(Move, i32)> {
        self.start = Instant::now();
        self.nodes = 0;
        self.stopped = false;
        self.limits = Limits { nodes: node_budget, ..Default::default() };
        let mut out = Vec::with_capacity(moves.len());
        self.rep.push(b.hash);
        for &m in moves {
            let child = b.make(m);
            let s = if depth == 0 {
                -self.quiesce(&child, -INF, INF, 1)
            } else {
                -self.negamax(&child, depth as i32 - 1, -INF, INF, 1, true)
            };
            out.push((m, s));
        }
        self.rep.pop();
        out
    }
}

fn has_non_pawn_material(b: &Board) -> bool {
    b.sq.iter().any(|&p| p != 0 && piece_color(p) == b.side && matches!(piece_type(p), KNIGHT | BISHOP | ROOK | QUEEN))
}

fn score_to_tt(s: i32, ply: usize) -> i32 {
    if s > MATE - 1000 {
        s + ply as i32
    } else if s < -MATE + 1000 {
        s - ply as i32
    } else {
        s
    }
}

fn score_from_tt(s: i32, ply: usize) -> i32 {
    if s > MATE - 1000 {
        s - ply as i32
    } else if s < -MATE + 1000 {
        s + ply as i32
    } else {
        s
    }
}

pub fn score_string(s: i32) -> String {
    if is_mate_score(s) {
        let plies = MATE - s.abs();
        let moves = (plies + 1) / 2;
        format!("mate {}", if s > 0 { moves } else { -moves })
    } else {
        format!("cp {s}")
    }
}

pub fn info_line(r: &SearchResult) -> String {
    let nps = if r.elapsed_ms > 0 { r.nodes * 1000 / r.elapsed_ms } else { 0 };
    let pv: Vec<String> = r.pv.iter().map(|m| m.uci()).collect();
    format!(
        "info depth {} score {} nodes {} nps {} time {} pv {}",
        r.depth,
        score_string(r.score),
        r.nodes,
        nps,
        r.elapsed_ms,
        pv.join(" ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn best(fen: &str, depth: u32) -> (String, i32) {
        let b = Board::from_fen(fen).unwrap();
        let mut s = Searcher::new(16);
        s.silent = true;
        let r = s.search(&b, Limits { depth: Some(depth), ..Default::default() });
        (r.best.uci(), r.score)
    }

    #[test]
    fn mate_in_one() {
        let (m, s) = best("6k1/5ppp/8/8/8/8/5PPP/3R2K1 w - - 0 1", 3);
        assert_eq!(m, "d1d8");
        assert!(is_mate_score(s));
    }

    #[test]
    fn mate_in_two() {
        // 1. Kb6 Kb8 2. Rh8# (or 1. Kc7 Ka7 2. Ra1#): mate in 2 = 3 plies.
        let (_, s) = best("k7/8/2K5/8/8/8/8/7R w - - 0 1", 6);
        assert_eq!(s, MATE - 3);
    }

    #[test]
    fn wins_hanging_queen() {
        let (m, _) = best("4k3/8/8/3q4/8/8/3R4/4K3 w - - 0 1", 4);
        assert_eq!(m, "d2d5");
    }

    #[test]
    fn avoids_stalemate_when_winning() {
        let b = Board::from_fen("7k/8/6Q1/8/8/8/8/6K1 w - - 0 1").unwrap();
        let mut s = Searcher::new(16);
        s.silent = true;
        let r = s.search(&b, Limits { depth: Some(6), ..Default::default() });
        let after = b.make(r.best);
        assert!(!after.legal_moves().is_empty() || after.in_check(), "played stalemate {}", r.best);
    }
}
