//! Maia as HL's gut feeling. Optional: HL_MAIA_LC0=<lc0 binary>, HL_MAIA_DIR=<folder with
//! maia-1100.pb.gz .. maia-1900.pb.gz>. Asks the rating-matched net for its move policy
//! (one node, no search) and hands back log-probabilities for the intuition stage.
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Mutex, OnceLock};

struct Net {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

static NETS: OnceLock<Mutex<HashMap<u32, Option<Net>>>> = OnceLock::new();

fn spawn(level: u32) -> Option<Net> {
    let lc0 = std::env::var("HL_MAIA_LC0").ok().filter(|s| !s.is_empty())?;
    let dir = std::env::var("HL_MAIA_DIR").ok().filter(|s| !s.is_empty())?;
    let mut child = Command::new(lc0).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let stdin = child.stdin.take()?;
    let stdout = BufReader::new(child.stdout.take()?);
    let mut n = Net { _child: child, stdin, stdout };
    n.send(&format!("setoption name WeightsFile value {dir}/maia-{level}.pb.gz"));
    n.send("setoption name VerboseMoveStats value true");
    n.send("setoption name Threads value 1");
    n.send("isready");
    n.wait_for("readyok")?;
    Some(n)
}

impl Net {
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
            if line.starts_with(tok) {
                return Some(());
            }
        }
    }

    fn policy(&mut self, fen: &str) -> Option<HashMap<String, f32>> {
        self.send(&format!("position fen {fen}"));
        self.send("go nodes 1");
        let mut out = HashMap::new();
        let mut line = String::new();
        loop {
            line.clear();
            if self.stdout.read_line(&mut line).ok()? == 0 {
                return None;
            }
            if line.starts_with("bestmove") {
                return Some(out);
            }
            // info string e7e5  (322 ) N: ... (P: 36.67%) ...
            if let Some(rest) = line.strip_prefix("info string ") {
                let mv = rest.split_whitespace().next().unwrap_or("");
                if mv == "node" {
                    continue;
                }
                if let Some(i) = rest.find("(P:") {
                    let p: String = rest[i + 3..].chars().take_while(|&c| c != '%').collect();
                    if let Ok(p) = p.trim().parse::<f32>() {
                        out.insert(mv.to_string(), p / 100.0);
                    }
                }
            }
        }
    }
}

pub fn enabled() -> bool {
    std::env::var("HL_MAIA_LC0").map_or(false, |s| !s.is_empty())
}

/// Maia's probability for each of `moves` (uci) in `fen`, from the net closest to `elo`.
pub fn probs(fen: &str, moves: &[String], elo: f32) -> Option<Vec<f32>> {
    let level = ((elo / 100.0).round() as u32 * 100).clamp(1100, 1900);
    let nets = NETS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut nets = nets.lock().ok()?;
    let net = nets.entry(level).or_insert_with(|| spawn(level)).as_mut()?;
    let pol = net.policy(fen)?;
    Some(moves.iter().map(|m| *pol.get(m).unwrap_or(&0.0)).collect())
}
