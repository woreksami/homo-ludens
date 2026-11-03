//! UCI protocol front end.
//!
//! Options:
//!   UCI_LimitStrength (default true)  human mode on/off
//!   UCI_Elo 100..3200 (default 1500)  the rating dial
//!   Seed (default 0 = vary per game)  makes a persona's choices reproducible
//!   ShowThoughts (default true)       print the thought trace as `info string`
//!   Hash (MB)
//!
//! In human mode every move also prints `info string thinktime <ms>`: the
//! simulated human think time, which the harness compares with real clocks.

use crate::board::*;
use crate::mind::{Mind, Persona};
use crate::search::{Limits, Searcher};
use std::io::{self, BufRead, Write};

pub struct Engine {
    pub board: Board,
    pub history: Vec<u64>,
    pub searcher: Searcher,
    pub human: bool,
    pub elo: f32,
    pub seed: u64,
    pub show_thoughts: bool,
    game_salt: u64,
}

impl Engine {
    pub fn new() -> Engine {
        let salt = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Engine {
            board: Board::startpos(),
            history: vec![],
            searcher: Searcher::new(32),
            human: true,
            elo: 1500.0,
            seed: 0,
            show_thoughts: true,
            game_salt: salt,
        }
    }

    fn position(&mut self, args: &[&str]) {
        let mut i = 0;
        let mut board = Board::startpos();
        if args.first() == Some(&"fen") {
            let end = args.iter().position(|&a| a == "moves").unwrap_or(args.len());
            match Board::from_fen(&args[1..end].join(" ")) {
                Ok(b) => board = b,
                Err(e) => {
                    println!("info string {e}");
                    return;
                }
            }
            i = end;
        } else if args.first() == Some(&"startpos") {
            i = 1;
        }
        self.history.clear();
        if args.get(i) == Some(&"moves") {
            for s in &args[i + 1..] {
                match board.parse_uci(s) {
                    Some(m) => {
                        self.history.push(board.hash);
                        board = board.make(m);
                    }
                    None => {
                        println!("info string illegal move {s}");
                        break;
                    }
                }
            }
        }
        self.board = board;
    }

    fn limits_from_go(&self, args: &[&str]) -> Limits {
        let mut l = Limits::default();
        let get = |k: &str| args.iter().position(|&a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok());
        l.depth = get("depth").map(|d| d as u32);
        l.nodes = get("nodes");
        l.movetime_ms = get("movetime");
        let (time, inc) = if self.board.side == WHITE { (get("wtime"), get("winc")) } else { (get("btime"), get("binc")) };
        if let Some(t) = time {
            let moves_to_go = get("movestogo").unwrap_or(30).max(1);
            l.movetime_ms = Some((t / moves_to_go + inc.unwrap_or(0) * 3 / 4).min(t.saturating_sub(50)).max(10));
        }
        if args.contains(&"infinite") || (l.depth.is_none() && l.nodes.is_none() && l.movetime_ms.is_none()) {
            l.movetime_ms = Some(l.movetime_ms.unwrap_or(5_000));
        }
        if let Some(i) = args.iter().position(|&a| a == "searchmoves") {
            let ms: Vec<Move> = args[i + 1..].iter().filter_map(|s| self.board.parse_uci(s)).collect();
            if !ms.is_empty() {
                l.root_moves = Some(ms);
            }
        }
        l
    }

    pub fn go(&mut self, args: &[&str]) -> Move {
        let limits = self.limits_from_go(args);
        self.searcher.set_game_history(self.history.clone());
        if !self.human {
            let r = self.searcher.search(&self.board, limits);
            return r.best;
        }
        let seed = if self.seed != 0 { self.seed } else { self.game_salt } ^ self.board.hash;
        let mut mind = Mind::new(Persona::from_elo(self.elo), seed, &mut self.searcher);
        let t = mind.think(&self.board, &limits);
        if self.show_thoughts {
            for line in &t.trace {
                println!("info string {line}");
            }
        }
        println!("info string thinktime {}", t.think_ms);
        t.best
    }

    fn set_option(&mut self, args: &[&str]) {
        let name_i = args.iter().position(|&a| a == "name");
        let value_i = args.iter().position(|&a| a == "value");
        let (Some(n), Some(v)) = (name_i, value_i) else { return };
        let name = args[n + 1..v].join(" ").to_lowercase();
        let value = args[v + 1..].join(" ");
        match name.as_str() {
            "uci_limitstrength" => self.human = value == "true",
            "uci_elo" => self.elo = value.parse().unwrap_or(self.elo),
            "seed" => self.seed = value.parse().unwrap_or(0),
            "showthoughts" => self.show_thoughts = value == "true",
            "hash" => self.searcher = Searcher::new(value.parse().unwrap_or(32)),
            _ => println!("info string unknown option {name}"),
        }
    }

    pub fn run(&mut self) {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            let args: Vec<&str> = line.split_whitespace().collect();
            let Some(&cmd) = args.first() else { continue };
            match cmd {
                "uci" => {
                    println!("id name Homo Ludens 0.1");
                    println!("id author Homo Ludens project");
                    println!("option name UCI_LimitStrength type check default true");
                    println!("option name UCI_Elo type spin default 1500 min 100 max 3200");
                    println!("option name Seed type spin default 0 min 0 max 2147483647");
                    println!("option name ShowThoughts type check default true");
                    println!("option name Hash type spin default 32 min 1 max 4096");
                    println!("uciok");
                }
                "isready" => println!("readyok"),
                "ucinewgame" => {
                    self.searcher.clear();
                    self.game_salt = self.game_salt.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                }
                "setoption" => self.set_option(&args[1..]),
                "position" => self.position(&args[1..]),
                "go" => {
                    let m = self.go(&args[1..]);
                    println!("bestmove {}", m.uci());
                }
                "d" => println!("{}", self.board),
                "perft" => {
                    let d = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4);
                    let t = std::time::Instant::now();
                    let n = self.board.perft(d);
                    println!("perft {d}: {n} nodes in {} ms", t.elapsed().as_millis());
                }
                "quit" => break,
                _ => {}
            }
            io::stdout().flush().ok();
        }
    }
}
