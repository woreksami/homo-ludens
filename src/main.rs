//! Homo Ludens: a classical chess engine whose reasoning models a human mind.
//!
//! Usage:
//!   homo_ludens                         UCI mode (for GUIs, cutechess, the harness)
//!   homo_ludens think <elo> [fen]       print one full thought process
//!   homo_ludens bench [depth]           engine speed and node count
//!   homo_ludens selfplay <eloA> <eloB> [games]   quick persona-vs-persona match

mod board;
mod eval;
mod mind;
mod search;
mod uci;

use board::*;
use mind::{Mind, Persona};
use search::{Limits, Searcher};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("think") => {
            let elo: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1500.0);
            let fen = if args.len() > 3 { args[3..].join(" ") } else { START_FEN.to_string() };
            let b = Board::from_fen(&fen).expect("valid FEN");
            let mut s = Searcher::new(32);
            let mut m = Mind::new(Persona::from_elo(elo), 42 ^ b.hash, &mut s);
            let t = m.think(&b, &Limits { movetime_ms: Some(2000), ..Default::default() });
            println!("{b}\n");
            for line in t.trace {
                println!("{line}");
            }
        }
        Some("bench") => bench(args.get(2).and_then(|s| s.parse().ok()).unwrap_or(9)),
        Some("selfplay") => {
            let a: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(800.0);
            let b: f32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(1600.0);
            let n: u32 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(20);
            selfplay(a, b, n);
        }
        _ => uci::Engine::new().run(),
    }
}

fn bench(depth: u32) {
    let fens = [
        START_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "r1bq1rk1/pp2bppp/2n1pn2/3p4/2PP4/2NBPN2/PP3PPP/R1BQK2R w KQ - 0 8",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "2r3k1/pp3ppp/4p3/3pP3/3P4/P4N2/1P3PPP/2R3K1 w - - 0 25",
    ];
    let mut total_nodes = 0u64;
    let t = std::time::Instant::now();
    for fen in fens {
        let b = Board::from_fen(fen).unwrap();
        let mut s = Searcher::new(32);
        s.silent = true;
        let r = s.search(&b, Limits { depth: Some(depth), ..Default::default() });
        println!("{:<75} {:>6} {:>10} nodes  best {}", fen, search::score_string(r.score), r.nodes, r.best);
        total_nodes += r.nodes;
    }
    let ms = t.elapsed().as_millis().max(1) as u64;
    println!("total {total_nodes} nodes in {ms} ms = {} nps", total_nodes * 1000 / ms);
}

/// Result from White's point of view: 1.0, 0.5 or 0.0.
fn play_game(white: f32, black: f32, seed: u64, opening: &[&str]) -> f32 {
    let mut b = Board::startpos();
    let mut hist: Vec<u64> = vec![];
    for u in opening {
        let m = b.parse_uci(u).unwrap();
        hist.push(b.hash);
        b = b.make(m);
    }
    let mut s = Searcher::new(16);
    s.silent = true;
    for ply in 0..400 {
        let moves = b.legal_moves();
        if moves.is_empty() {
            return if b.in_check() { if b.side == WHITE { 0.0 } else { 1.0 } } else { 0.5 };
        }
        if b.halfmove >= 100 || b.insufficient_material() || hist.iter().filter(|&&h| h == b.hash).count() >= 2 {
            return 0.5;
        }
        let elo = if b.side == WHITE { white } else { black };
        s.set_game_history(hist.clone());
        let mut mind = Mind::new(Persona::from_elo(elo), seed ^ b.hash ^ ply as u64, &mut s);
        let t = mind.think(&b, &Limits { movetime_ms: Some(200), ..Default::default() });
        hist.push(b.hash);
        b = b.make(t.best);
    }
    0.5
}

fn selfplay(a: f32, bb: f32, games: u32) {
    let openings: [&[&str]; 6] = [
        &["e2e4", "e7e5"],
        &["d2d4", "d7d5"],
        &["e2e4", "c7c5"],
        &["d2d4", "g8f6"],
        &["c2c4", "e7e5"],
        &["e2e4", "e7e6"],
    ];
    let mut score_a = 0.0;
    for g in 0..games {
        let opening = openings[(g / 2) as usize % openings.len()];
        let a_white = g % 2 == 0;
        let r = if a_white { play_game(a, bb, g as u64 * 7919, opening) } else { 1.0 - play_game(bb, a, g as u64 * 7919, opening) };
        score_a += r;
        println!("game {:>3}: {} {}", g + 1, if a_white { "A white" } else { "A black" }, r);
    }
    let pct = score_a / games as f32;
    let diff = if pct <= 0.0 || pct >= 1.0 { f32::NAN } else { -400.0 * (1.0 / pct - 1.0).log10() };
    println!("A ({a}) scored {score_a}/{games} vs B ({bb}): {:.0}% , performance diff {:+.0}", pct * 100.0, diff);
}
