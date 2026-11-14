//! UCI front end. Options: UCI_LimitStrength (human mode), UCI_Elo 100-3200,
//! Seed, ShowThoughts, SimTimeControl, RawSkill, BookFile, Hash.
//! Human mode also prints `info string thinktime <ms>` every move.

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
    pub raw_skill: bool,
    // live play (lichess etc): search like the evals do, then hold the move until the human think time is up
    pub real_time: bool,
    pub tc_scale: f32,
    last_capture_sq: Option<u8>,
    last_to: Option<u8>,
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
            raw_skill: false,
            real_time: false,
            tc_scale: 1.0,
            last_capture_sq: None,
            last_to: None,
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
        self.last_capture_sq = None;
        self.last_to = None;
        if args.get(i) == Some(&"moves") {
            for s in &args[i + 1..] {
                match board.parse_uci(s) {
                    Some(m) => {
                        self.last_capture_sq = if board.sq[m.to as usize] != 0 { Some(m.to) } else { None };
                        self.last_to = Some(m.to);
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
        // explicit movetime beats the clock
        if let (Some(t), None) = (time, l.movetime_ms) {
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
        #[cfg(not(target_arch = "wasm32"))]
        let started = std::time::Instant::now();
        let mut limits = self.limits_from_go(args);
        if self.real_time && self.human {
            // same budget the anti-cheat evals were run with (go movetime 500 + clock)
            let get = |k: &str| args.iter().position(|&a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<u64>().ok());
            let left = if self.board.side == WHITE { get("wtime") } else { get("btime") };
            limits.movetime_ms = Some(left.map_or(500, |t| (t / 20).clamp(20, 500)));
        }
        self.searcher.set_game_history(self.history.clone());
        if !self.human {
            let r = self.searcher.search(&self.board, limits);
            return r.best;
        }
        let seed = if self.seed != 0 { self.seed } else { self.game_salt } ^ self.board.hash;
        let mut mind = Mind::new(if self.raw_skill { Persona::from_internal(self.elo, self.elo) } else { Persona::from_elo(self.elo) }, seed, &mut self.searcher);
        mind.tc_scale = self.tc_scale;
        mind.last_capture_sq = self.last_capture_sq;
        mind.last_to = self.last_to;
        let get = |k: &str| args.iter().position(|&a| a == k).and_then(|i| args.get(i + 1)).and_then(|v| v.parse::<f32>().ok());
        let (time, inc) = if self.board.side == WHITE { (get("wtime"), get("winc")) } else { (get("btime"), get("binc")) };
        mind.clock = time.map(|t| (t / 1000.0, inc.unwrap_or(0.0) / 1000.0));
        let t = mind.think(&self.board, &limits);
        if self.show_thoughts {
            for line in &t.trace {
                println!("info string {line}");
            }
        }
        println!("info string thinktime {}", t.think_ms);
        if t.resign {
            println!("info string resign");
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.real_time {
            let wait = std::time::Duration::from_millis(t.think_ms);
            if let Some(rest) = wait.checked_sub(started.elapsed()) {
                std::thread::sleep(rest);
            }
        }
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
            // base+inc in seconds like PGN TimeControl
            "simtimecontrol" => {
                let mut it = value.split('+').map(|x| x.trim().parse::<f32>().unwrap_or(0.0));
                let (base, inc) = (it.next().unwrap_or(300.0), it.next().unwrap_or(0.0));
                if base + inc > 0.0 {
                    self.tc_scale = (base + 40.0 * inc) / 300.0;
                }
            }
            "rawskill" => self.raw_skill = value == "true",
            "realtime" => self.real_time = value == "true",
            "bookfile" => crate::mind::book::init(&value),
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
                    println!("option name SimTimeControl type string default 300+0");
                    println!("option name RawSkill type check default false");
                    println!("option name RealTime type check default false");
                    println!("option name BookFile type string default data/book.tsv");
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
                // custom: static eval + concepts, white POV
                "eval" => {
                    let bd = crate::eval::breakdown(&self.board);
                    println!("eval {} {:?}", bd.total(), bd.terms);
                }
                // custom: `evalmoves [depth]` -> e2e4:31 d2d4:28 ... (stm POV, cp)
                "evalmoves" => {
                    let d = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(2);
                    self.searcher.set_game_history(self.history.clone());
                    let moves = self.board.legal_moves();
                    let scored = self.searcher.score_moves(&self.board, &moves, d, Some(200_000), None);
                    let parts: Vec<String> = scored.iter().map(|(m, s)| format!("{}:{}", m.uci(), s)).collect();
                    println!("evalmoves {}", parts.join(" "));
                }
                // custom: `natdump <in> <out>`, lines are fen<TAB>last_to|-<TAB>played
                "natdump" => {
                    use std::io::Write;
                    let text = std::fs::read_to_string(args[1]).unwrap_or_default();
                    let mut out = std::io::BufWriter::new(std::fs::File::create(args[2]).unwrap());
                    for line in text.lines() {
                        let f: Vec<&str> = line.split('\t').collect();
                        let Ok(b) = Board::from_fen(f[0]) else { continue };
                        let last = crate::board::parse_sq(f[1]);
                        // last col = shallow search score so the fit can tell natural from just good
                        let moves = b.legal_moves();
                        self.searcher.set_game_history(vec![]);
                        let scored = self.searcher.score_moves(&b, &moves, 1, Some(50_000), None);
                        for (m, sc) in scored {
                            let x = crate::mind::natural::features(&b, m, last);
                            let xs: Vec<String> = x.iter().map(|v| format!("{v:.3}")).collect();
                            writeln!(out, "{} {} {}", (m.uci() == f[2]) as u8, xs.join(" "), sc.clamp(-2000, 2000)).unwrap();
                        }
                        writeln!(out).unwrap();
                    }
                    println!("natdump done");
                }
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
