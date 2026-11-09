//! Optional external UCI engine ("oracle") for the deep-calculation stage at top
//! settings. Enabled by HL_ORACLE=<path to engine>; HL_ORACLE_NODES sets the
//! node budget per query (default 150000). The human layer still chooses; the
//! oracle only replaces HL's own search where it challenges that choice.
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};

pub struct Oracle {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    nodes: u64,
}

static ORACLE: OnceLock<Option<Mutex<Oracle>>> = OnceLock::new();

fn start() -> Option<Mutex<Oracle>> {
    let path = std::env::var("HL_ORACLE").ok().filter(|p| !p.is_empty())?;
    let nodes = std::env::var("HL_ORACLE_NODES").ok().and_then(|v| v.parse().ok()).unwrap_or(150_000);
    let mut child = Command::new(path).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let stdin = child.stdin.take()?;
    let stdout = BufReader::new(child.stdout.take()?);
    let mut o = Oracle { _child: child, stdin, stdout, nodes };
    o.send("uci");
    o.wait_for("uciok")?;
    o.send("setoption name Threads value 1");
    o.send("setoption name Hash value 64");
    o.send("isready");
    o.wait_for("readyok")?;
    Some(Mutex::new(o))
}

pub fn get() -> Option<&'static Mutex<Oracle>> {
    ORACLE.get_or_init(start).as_ref()
}

impl Oracle {
    fn send(&mut self, s: &str) {
        let _ = writeln!(self.stdin, "{s}");
        let _ = self.stdin.flush();
    }

    fn wait_for(&mut self, tok: &str) -> Option<()> {
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).ok()? == 0 {
                return None;
            }
            if line.trim_start().starts_with(tok) {
                return Some(());
            }
        }
    }

    /// Search `fen`, optionally restricted to one move. Returns (best move in UCI, score cp for side to move).
    pub fn go(&mut self, fen: &str, only: Option<&str>) -> Option<(String, i32)> {
        self.send(&format!("position fen {fen}"));
        match only {
            Some(m) => self.send(&format!("go nodes {} searchmoves {m}", self.nodes)),
            None => self.send(&format!("go nodes {}", self.nodes)),
        }
        let mut score = 0;
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).ok()? == 0 {
                return None;
            }
            let t: Vec<&str> = line.split_whitespace().collect();
            if t.first() == Some(&"info") {
                if let Some(i) = t.iter().position(|&x| x == "score") {
                    match (t.get(i + 1), t.get(i + 2).and_then(|v| v.parse::<i32>().ok())) {
                        (Some(&"cp"), Some(v)) => score = v,
                        (Some(&"mate"), Some(v)) => score = if v > 0 { 30000 - v } else { -30000 - v },
                        _ => {}
                    }
                }
            } else if t.first() == Some(&"bestmove") {
                return Some((t.get(1)?.to_string(), score));
            }
        }
    }
}
