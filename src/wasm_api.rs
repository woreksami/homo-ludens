//! WebAssembly API for playing Homo Ludens in a browser.
//!
//! Plain C ABI, no bindings generator: the page writes a UTF-8 request into
//! memory from `hl_alloc`, calls a function, and reads a NUL-terminated
//! reply. A position request is the start FEN on the first line and the
//! moves played (UCI) on the second.
//!
//! - `hl_position` -> `fen`, `legal` (UCI moves), `san` (same order),
//!   `check`, `status` (`play`, `checkmate`, `stalemate`, `draw-50`,
//!   `draw-repetition`, `draw-material`) lines.
//! - `hl_think(elo, seed, clock_ms, inc_ms)` -> `move`, `think` (simulated
//!   ms), `resign`, then `trace` lines: the engine's own thoughts.

use crate::board::*;
use crate::mind::{book, notation::san, Mind, Persona};
use crate::search::{Limits, Searcher};
use std::sync::Mutex;

static OUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static IN: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static SEARCHER: Mutex<Option<Searcher>> = Mutex::new(None);

fn reply(s: String) -> *const u8 {
    let mut o = OUT.lock().unwrap();
    o.clear();
    o.extend_from_slice(s.as_bytes());
    o.push(0);
    o.as_ptr()
}

/// Space for a request of `len` bytes; the page writes it there.
#[unsafe(no_mangle)]
pub extern "C" fn hl_alloc(len: usize) -> *mut u8 {
    let mut i = IN.lock().unwrap();
    i.clear();
    i.resize(len, 0);
    i.as_mut_ptr()
}

struct Pos {
    board: Board,
    history: Vec<u64>,
    last_capture_sq: Option<u8>,
    last_to: Option<u8>,
}

fn read_position() -> Result<Pos, String> {
    let i = IN.lock().unwrap();
    let text = std::str::from_utf8(&i).map_err(|_| "bad utf-8".to_string())?;
    let mut lines = text.lines();
    let fen = lines.next().unwrap_or("").trim();
    let mut board = if fen.is_empty() || fen == "startpos" { Board::startpos() } else { Board::from_fen(fen)? };
    let mut history = vec![];
    let mut last_capture_sq = None;
    let mut last_to = None;
    for s in lines.next().unwrap_or("").split_whitespace() {
        let m = board.parse_uci(s).ok_or(format!("illegal move {s}"))?;
        last_capture_sq = if board.sq[m.to as usize] != 0 { Some(m.to) } else { None };
        last_to = Some(m.to);
        history.push(board.hash);
        board = board.make(m);
    }
    Ok(Pos { board, history, last_capture_sq, last_to })
}

fn insufficient_material(b: &Board) -> bool {
    let mut minors = 0;
    for &p in b.sq.iter() {
        match p & 7 {
            0 | KING => {}
            KNIGHT | BISHOP => minors += 1,
            _ => return false,
        }
    }
    minors <= 1
}

fn status(p: &Pos, legal: &[Move]) -> &'static str {
    let b = &p.board;
    if legal.is_empty() {
        return if b.in_check() { "checkmate" } else { "stalemate" };
    }
    if b.halfmove >= 100 {
        return "draw-50";
    }
    if p.history.iter().filter(|&&h| h == b.hash).count() >= 2 {
        return "draw-repetition";
    }
    if insufficient_material(b) {
        return "draw-material";
    }
    "play"
}

#[unsafe(no_mangle)]
pub extern "C" fn hl_position() -> *const u8 {
    let p = match read_position() {
        Ok(p) => p,
        Err(e) => return reply(format!("error {e}\n")),
    };
    let legal = p.board.legal_moves();
    let ucis: Vec<String> = legal.iter().map(|m| m.uci()).collect();
    let sans: Vec<String> = legal.iter().map(|m| san(&p.board, *m)).collect();
    reply(format!(
        "fen {}\nlegal {}\nsan {}\ncheck {}\nstatus {}\n",
        p.board.to_fen(),
        ucis.join(" "),
        sans.join(" "),
        p.board.in_check() as u8,
        status(&p, &legal)
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn hl_think(elo: f32, seed: u32, clock_ms: f64, inc_ms: f64) -> *const u8 {
    let p = match read_position() {
        Ok(p) => p,
        Err(e) => return reply(format!("error {e}\n")),
    };
    if book::get().is_none() {
        book::init_text(include_str!("../data/book.tsv"));
    }
    let mut guard = SEARCHER.lock().unwrap();
    let searcher = guard.get_or_insert_with(|| Searcher::new(16));
    searcher.set_game_history(p.history.clone());
    let seed = (seed as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ p.board.hash;
    let mut mind = Mind::new(Persona::from_elo(elo), seed, searcher);
    if clock_ms > 0.0 {
        mind.clock = Some(((clock_ms / 1000.0) as f32, (inc_ms / 1000.0) as f32));
        let base_guess = clock_ms / 1000.0 + 40.0 * inc_ms / 1000.0;
        mind.tc_scale = (base_guess / 300.0).clamp(0.1, 20.0) as f32;
    }
    mind.last_capture_sq = p.last_capture_sq;
    mind.last_to = p.last_to;
    let limits = Limits { movetime_ms: Some(400), ..Limits::default() };
    let t = mind.think(&p.board, &limits);
    let mut out = format!("move {}\nthink {}\nresign {}\n", t.best.uci(), t.think_ms, t.resign as u8);
    for line in t.trace {
        out.push_str("trace ");
        out.push_str(&line.replace('\n', " "));
        out.push('\n');
    }
    reply(out)
}

/// A new game: forget the transposition table and search history.
#[unsafe(no_mangle)]
pub extern "C" fn hl_new_game() {
    if let Some(s) = SEARCHER.lock().unwrap().as_mut() {
        s.clear();
    }
}
