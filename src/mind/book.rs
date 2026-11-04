//! Opening memory: what players of a given rating actually played here.
//!
//! Loaded from the TSV `harness/build_book.py` writes. Counts from the
//! player's own rating band count fully, neighbouring bands less, so a
//! 1400 player mostly remembers 1400 openings but has heard of others.

use crate::board::{Board, Move};
use crate::mind::persona::Rng;
use std::collections::HashMap;
use std::sync::OnceLock;

pub struct Book {
    /// key -> [(band, uci, count)]
    entries: HashMap<String, Vec<(i32, String, u32)>>,
}

static BOOK: OnceLock<Option<Book>> = OnceLock::new();

pub fn key(b: &Board) -> String {
    b.to_fen().split_whitespace().take(3).collect::<Vec<_>>().join(" ")
}

impl Book {
    pub fn load(path: &str) -> Option<Book> {
        Book::parse(&std::fs::read_to_string(path).ok()?)
    }

    pub fn parse(text: &str) -> Option<Book> {
        let mut entries: HashMap<String, Vec<(i32, String, u32)>> = HashMap::new();
        for line in text.lines() {
            let mut it = line.split('\t');
            let (Some(band), Some(fen), Some(moves)) = (it.next(), it.next(), it.next()) else { continue };
            let Ok(band) = band.parse::<i32>() else { continue };
            let e = entries.entry(fen.to_string()).or_default();
            for mc in moves.split(' ') {
                if let Some((u, c)) = mc.split_once(':') {
                    e.push((band, u.to_string(), c.parse().unwrap_or(0)));
                }
            }
        }
        Some(Book { entries })
    }

    /// Weighted human move counts for this position at this rating.
    pub fn weights(&self, b: &Board, elo: f32) -> Vec<(Move, f32)> {
        let Some(e) = self.entries.get(&key(b)) else { return vec![] };
        let mut w: Vec<(Move, f32)> = vec![];
        for (band, u, c) in e {
            let dist = ((*band as f32 + 100.0) - elo).abs() / 200.0;
            let k = (-dist * dist / 2.0).exp(); // ~1 own band, 0.6 next, 0.14 two away
            let Some(m) = b.parse_uci(u) else { continue };
            match w.iter_mut().find(|x| x.0 == m) {
                Some(x) => x.1 += k * *c as f32,
                None => w.push((m, k * *c as f32)),
            }
        }
        w
    }

    /// Sample a remembered move, if the memory is strong enough.
    pub fn pick(&self, b: &Board, elo: f32, rng: &mut Rng) -> Option<(Move, f32)> {
        let w = self.weights(b, elo);
        let total: f32 = w.iter().map(|x| x.1).sum();
        if total < 1.5 {
            return None;
        }
        let mut r = rng.f32() * total;
        for (m, x) in &w {
            r -= x;
            if r <= 0.0 {
                return Some((*m, total));
            }
        }
        w.last().map(|x| (x.0, total))
    }
}

pub fn init(path: &str) {
    let _ = BOOK.set(Book::load(path));
}

/// Use an opening book given as text (the browser build embeds it).
pub fn init_text(text: &str) {
    let _ = BOOK.set(Book::parse(text));
}

pub fn get() -> Option<&'static Book> {
    BOOK.get_or_init(|| Book::load("data/book.tsv")).as_ref()
}
