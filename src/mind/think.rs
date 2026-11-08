//! Stages 3-7: intuition, deliberation, evaluation, verification, decision.
//!
//! `Mind::think` runs one human "thought process" for one move and returns the
//! move, the simulated think time and a readable trace of how it got there.

use super::notation::{san, san_line};
use super::perception::{en_prise, perceive, perceive_replies, Percept, Reason};
use super::persona::{env_scale, Persona, Rng};
use crate::board::*;
use crate::eval::{self, CONCEPT_NAMES, N_CONCEPTS};
use crate::search::{self, Limits, Searcher, INF, MATE};

#[derive(Clone, Debug)]
pub struct Candidate {
    pub mv: Move,
    pub intuition: i32,
    /// Value after the player's own calculation (their point of view).
    pub value: i32,
    pub line: Vec<Move>,
    pub first_idea: bool,
}

#[derive(Clone, Debug)]
pub struct Thought {
    pub best: Move,
    pub think_ms: u64,
    pub trace: Vec<String>,
    pub candidates: Vec<Candidate>,
    pub nodes: u32,
    /// The player gives up: the position has felt hopeless for long enough.
    pub resign: bool,
}

pub struct Mind<'a> {
    pub persona: Persona,
    rng: Rng,
    seed: u64,
    nodes: u32,
    budget: u32,
    /// Time-control scale for simulated think time (1.0 = 5-minute game).
    pub tc_scale: f32,
    /// Simulated clock (remaining seconds, increment seconds), when playing on one.
    pub clock: Option<(f32, f32)>,
    /// Square the opponent just captured on, for spotting recaptures.
    pub last_capture_sq: Option<u8>,
    /// Square the opponent's last move went to.
    pub last_to: Option<u8>,
    /// Care multiplier for this move: players are careful when the game is balanced
    /// and sloppier once it is decided (scaled error grows with the evaluation).
    sloppy: f32,
    press: f32,
    searcher: &'a mut Searcher,
}

impl<'a> Mind<'a> {
    pub fn new(persona: Persona, seed: u64, searcher: &'a mut Searcher) -> Mind<'a> {
        Mind { persona, rng: Rng::new(seed), seed, nodes: 0, budget: 0, tc_scale: 1.0, clock: None, last_capture_sq: None, last_to: None, sloppy: 1.0, press: 0.0, searcher }
    }

    /// How the position "feels" to this player, from the side to move's view.
    /// Only known concepts count, and the judgement is noisy but stable for a
    /// given position (the same player misjudges the same position the same way).
    pub fn feel(&self, b: &Board) -> i32 {
        let bd = eval::breakdown(b);
        let mut w = self.persona.knowledge;
        w[eval::Concept::Material as usize] = 1.0;
        let mut v = bd.weighted(&w);
        let mut r = Rng::new(b.hash ^ self.seed.rotate_left(17));
        // Heavy-tailed misjudgement: most positions are read about right, a few
        // are badly misread (same overall variance as a plain gaussian).
        let g = r.gauss();
        let scale = if r.f32() < 0.12 { 2.45 } else { 0.62 };
        v += (g * scale * self.persona.eval_noise_cp * self.sloppy) as i32;
        if b.side == WHITE {
            v
        } else {
            -v
        }
    }

    /// Which concepts dominated the player's judgement of a position (for the trace).
    fn main_concepts(&self, b: &Board) -> String {
        let bd = eval::breakdown(b);
        let mut items: Vec<(usize, f32)> = (0..N_CONCEPTS)
            .map(|i| (i, bd.terms[i] as f32 * if i == 0 { 1.0 } else { self.persona.knowledge[i] }))
            .filter(|(_, v)| v.abs() >= 15.0)
            .collect();
        items.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
        let us = if b.side == WHITE { 1.0 } else { -1.0 };
        items
            .iter()
            .take(2)
            .map(|&(i, v)| format!("{} {}", CONCEPT_NAMES[i], if v * us > 0.0 { "favours me" } else { "favours them" }))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Human calculation: alpha-beta over the moves this player would imagine,
    /// with a stand-pat on their own judgement and working-memory lapses.
    fn calculate(&mut self, b: &Board, depth: u32, mut alpha: i32, beta: i32, forgotten: &mut Vec<u8>, line: &mut Vec<Move>, ply: i32) -> i32 {
        self.nodes += 1;
        let legal_empty = b.legal_moves().is_empty();
        if legal_empty {
            return if b.in_check() { -MATE + ply } else { 0 };
        }
        let in_check = b.in_check();
        let stand = self.feel(b);
        if depth == 0 || self.nodes >= self.budget {
            // At the end of a line a player still sees that hanging material can
            // be taken back ("...Qxd5, exd5"): resolve the best winning capture
            // they notice, instead of stopping on the opponent's grab.
            if !in_check {
                let mut gain = 0;
                for m in b.legal_captures() {
                    let g = crate::see::see(b, m);
                    if g > gain && self.rng.chance(self.persona.p_see_capture) {
                        gain = g;
                    }
                }
                return stand + gain;
            }
            return stand;
        }
        let mut best = if in_check { -INF } else { stand };
        if !in_check {
            if stand >= beta {
                return stand;
            }
            alpha = alpha.max(stand);
        }
        // In check, a player looks at every way out; otherwise only the replies
        // that come to mind. Nothing imagined means: trust the feel.
        let replies = if in_check { b.legal_moves() } else { perceive_replies(b, &self.persona, &mut self.rng, forgotten) };
        if replies.is_empty() {
            return stand;
        }
        let mut best_line: Vec<Move> = vec![];
        for m in replies {
            let mut sub = vec![m];
            let pushed = self.rng.chance(self.persona.p_forget);
            if pushed {
                forgotten.push(m.to);
            }
            let v = -self.calculate(&b.make(m), depth - 1, -beta, -alpha, forgotten, &mut sub, ply + 1);
            if pushed {
                forgotten.pop();
            }
            if v > best {
                best = v;
                best_line = sub;
            }
            if v > alpha {
                alpha = v;
                if alpha >= beta {
                    break;
                }
            }
        }
        line.extend(best_line);
        best
    }

    fn sample_candidates(&mut self, b: &Board, percepts: &[Percept], k: usize, first_round: bool) -> Vec<Candidate> {
        // Intuition: how good each move "looks" at a glance.
        // How natural each move looks to players of this rating (learned from their games).
        let mv: Vec<Move> = percepts.iter().map(|p| p.mv).collect();
        let nat = super::natural::log_probs(b, &mv, self.last_to, self.persona.elo);
        let nat_w = self.persona.naturalness_cp;
        let mut scored: Vec<(f32, &Percept)> = percepts
            .iter()
            .zip(nat.iter())
            .map(|(p, &lp)| {
                let after = b.make(p.mv);
                let mut s = -self.feel(&after) as f32 + nat_w * lp;
                s += p.salience * (self.persona.forcing_attraction + 25.0 * self.press);
                // "Can it just be taken?" Noticed as often as threats are.
                if let Some(loss) = en_prise(&after, p.mv.to) {
                    if self.rng.chance(self.persona.p_see_threat) {
                        s -= loss as f32;
                    }
                }
                for r in &p.reasons {
                    match r {
                        Reason::Pattern { bonus, .. } => s += *bonus as f32,
                        Reason::Mate => s += 5000.0,
                        _ => {}
                    }
                }
                (s, p)
            })
            .collect();

        // Sample without replacement, softmax at the persona's temperature.
        let k = k.max(1).min(scored.len());
        let temp = (0.8 * self.persona.intuition_temp * self.sloppy * (1.0 - 0.4 * self.press)).max(1.0);
        let mut chosen = vec![];
        for _ in 0..k {
            let maxs = scored.iter().map(|x| x.0).fold(f32::MIN, f32::max);
            let weights: Vec<f32> = scored.iter().map(|x| ((x.0 - maxs) / temp).exp()).collect();
            let total: f32 = weights.iter().sum();
            let mut pick = self.rng.f32() * total;
            let mut idx = weights.len() - 1;
            for (i, w) in weights.iter().enumerate() {
                if pick < *w {
                    idx = i;
                    break;
                }
                pick -= w;
            }
            let (s, p) = scored.remove(idx);
            chosen.push(Candidate { mv: p.mv, intuition: s as i32, value: s as i32, line: vec![], first_idea: first_round && chosen.is_empty() });
        }
        chosen
    }

    /// A careful look at the chosen move: can the opponent punish it right away?
    fn blunder_check(&mut self, b: &Board, c: &Candidate) -> Option<String> {
        let after = b.make(c.mv);
        let careful = Persona { p_blind_quiet: 1.0, p_blind_backward: 0.0, opp_replies: 12, p_forget: 0.0, ..self.persona.clone() };
        let saved = std::mem::replace(&mut self.persona, careful);
        let saved_budget = self.budget;
        self.budget = self.nodes + 400;
        let mut line = vec![];
        let v = -self.calculate(&after, 2, -INF, INF, &mut vec![], &mut line, 1);
        self.budget = saved_budget;
        self.persona = saved;
        if v < c.value - 150 && !line.is_empty() {
            Some(format!("{} looks bad after {} (feels like {})", san(b, c.mv), san_line(&after, &line), fmt_cp(v)))
        } else {
            None
        }
    }

    pub fn think(&mut self, b: &Board, limits: &Limits) -> Thought {
        let mut trace = vec![];
        let p = self.persona.clone();
        self.nodes = 0;
        self.budget = p.node_budget;
        // Time trouble: with little on the clock, players calculate less, and their
        // concrete checking (stage 6b) shrinks with it.
        let mut hurry = 1.0f32;
        // Time per move: a rapid player calculates more than a bullet player of the
        // same rating, so effort scales with the time this move can be given.
        let mut effort = 1.0f32;
        if let Some((rem, inc)) = self.clock {
            let moves_left = (40.0 - 0.6 * b.fullmove as f32).max(15.0);
            let allot = rem.max(0.0) / moves_left + 0.8 * inc;
            effort = (allot / 6.0).sqrt().clamp(0.6, 1.8);
            self.budget = ((self.budget as f32) * effort) as u32;
        }
        if let Some((rem, inc)) = self.clock {
            let t = rem + 10.0 * inc;
            if t < 15.0 {
                hurry = (t / 15.0).max(0.2);
                self.budget = ((p.node_budget as f32) * (t / 15.0).max(0.25)) as u32;
                trace.push(format!("Only {:.0} s left; no time to calculate deeply.", rem));
            }
        }
        trace.push(format!("Playing as a {:.0}-rated player.", p.elo));

        let legal = b.legal_moves();
        if legal.len() == 1 {
            trace.push(format!("Only one legal move: {}.", san(b, legal[0])));
            let ms = (350.0 + 300.0 * self.rng.f32()) as u64;
            return Thought { best: legal[0], think_ms: ms, trace, candidates: vec![], nodes: 0, resign: false };
        }

        // Stage 0: opening memory. Familiar positions are played from memory, fast.
        if let Some(book) = super::book::get() {
            if let Some((m, seen)) = book.pick(b, p.elo, &mut self.rng) {
                if legal.contains(&m) {
                    let ms = ((0.3 + 2.5 * self.rng.f32().powi(2)) * 1000.0 * self.tc_scale.clamp(0.1, 20.0).sqrt() / (1.0 + seen / 200.0).sqrt()) as u64;
                    trace.push(format!("I know this position; {} is what I usually play.", san(b, m)));
                    let floor = (400.0 + 150.0 * self.rng.f32()) as u64;
                    return Thought { best: m, think_ms: ms.max(floor), trace, candidates: vec![], nodes: 0, resign: false };
                }
            }
        }

        // Stage 1-2: perception.
        let percepts = perceive(b, &p, &mut self.rng);
        self.sloppy = 1.0;
        let feel_now = self.feel(b);
        // Players defend lost positions worse than they play level ones: once clearly
        // worse, attention and effort drop and errors grow.
        self.sloppy = 1.0 + 1.5 * ((-feel_now - 150) as f32 / 750.0).clamp(0.0, 1.0);
        // ...and smell blood once clearly winning: focused, forcing, going for the king.
        self.press = env_scale("HL_PRESS") * ((feel_now - 300) as f32 / 600.0).clamp(0.0, 1.0);
        let concepts = self.main_concepts(b);
        trace.push(format!(
            "I notice {} of {} legal moves. Position feels {}{}.",
            percepts.len(),
            legal.len(),
            fmt_cp(feel_now),
            if concepts.is_empty() { String::new() } else { format!(" ({concepts})") }
        ));
        for perc in percepts.iter().filter(|x| x.salience >= 1.0).take(4) {
            let why: Vec<String> = perc.reasons.iter().map(|r| r.describe()).collect();
            trace.push(format!("  {} catches my eye: {}.", san(b, perc.mv), why.join(", ")));
        }

        // Stage 3: intuition.
        let mut cands = self.sample_candidates(b, &percepts, p.max_candidates, true);
        let names: Vec<String> = cands.iter().map(|c| san(b, c.mv)).collect();
        trace.push(format!("Candidates: {}.", names.join(", ")));

        // Stage 4-5: progressive deepening, first idea favoured (Einstellung).
        let mut deepest = 0;
        for depth in 1..=p.calc_depth {
            if self.nodes >= self.budget {
                break;
            }
            deepest = depth;
            for c in cands.iter_mut() {
                let after = b.make(c.mv);
                let mut line = vec![];
                let mut forgotten = vec![];
                let v = -self.calculate_outer(&after, depth - 1, &mut forgotten, &mut line);
                c.value = v + if c.first_idea { p.einstellung_cp as i32 } else { 0 };
                c.line = line;
            }
            cands.sort_by_key(|c| -c.value);
            if cands.len() >= 2 && cands[0].value - cands[1].value > p.satisfice_cp as i32 && depth >= 2.max(p.calc_depth.min(8) / 2) {
                trace.push(format!("{} is clearly best to me; no need to look deeper.", san(b, cands[0].mv)));
                break;
            }
        }
        // Every idea disappoints: go back and look for something else.
        let mut rounds = 0;
        while p.calc_depth > 0
            && rounds < 2
            && cands[0].value < feel_now - 150
            && self.nodes < self.budget
            && self.rng.chance(p.p_rethink)
        {
            rounds += 1;
            let tried: Vec<Move> = cands.iter().map(|c| c.mv).collect();
            let fresh: Vec<Percept> = percepts.iter().filter(|x| !tried.contains(&x.mv)).cloned().collect();
            if fresh.is_empty() {
                break;
            }
            let mut more = self.sample_candidates(b, &fresh, p.max_candidates, false);
            for c in more.iter_mut() {
                let after = b.make(c.mv);
                let mut line = vec![];
                c.value = -self.calculate_outer(&after, deepest.max(1) - 1, &mut vec![], &mut line);
                c.line = line;
            }
            let names: Vec<String> = more.iter().map(|c| san(b, c.mv)).collect();
            trace.push(format!("Nothing I like ({} at best). Looking again: {}.", fmt_cp(cands[0].value), names.join(", ")));
            cands.extend(more);
            cands.sort_by_key(|c| -c.value);
        }
        if p.calc_depth == 0 {
            cands.sort_by_key(|c| -c.value);
            trace.push("I go with my first impression.".into());
        } else {
            trace.push(format!("Calculated {} plies deep, {} positions imagined.", deepest, self.nodes));
            for c in cands.iter().take(3) {
                let after = b.make(c.mv);
                let line = if c.line.is_empty() { String::new() } else { format!(" ({} {})", san(b, c.mv), san_line(&after, &c.line)) };
                trace.push(format!(
                    "  {}: {}{}{}",
                    san(b, c.mv),
                    fmt_cp(c.value),
                    line,
                    if c.first_idea && p.einstellung_cp > 10.0 { " [first idea]" } else { "" }
                ));
            }
        }

        // Repetition: a player who stands better avoids repeating; one who stands worse
        // is happy to. A position seen twice before is simply a draw.
        {
            let hist = self.searcher.game_history();
            let ahead = cands[0].value;
            let mut changed = false;
            for c in cands.iter_mut() {
                let h = b.make(c.mv).hash;
                let seen = hist.iter().filter(|&&x| x == h).count();
                if seen >= 2 {
                    c.value = if ahead > 50 { -30 } else { 0 };
                    changed = true;
                } else if seen == 1 && ahead > 100 {
                    c.value -= 60;
                    changed = true;
                }
            }
            if changed {
                cands.sort_by_key(|c| -c.value);
            }
        }

        // Stage 6a: blunder check.
        let mut chosen = 0;
        if self.rng.chance(p.p_blunder_check) {
            while chosen < cands.len() {
                match self.blunder_check(b, &cands[chosen].clone()) {
                    Some(msg) if chosen + 1 < cands.len() => {
                        trace.push(format!("Blunder check: {msg}. Rejecting it."));
                        chosen += 1;
                    }
                    _ => break,
                }
            }
            if chosen < cands.len() {
                trace.push(format!("Blunder check on {} passes.", san(b, cands[chosen].mv)));
            }
        } else if p.p_blunder_check > 0.0 {
            trace.push("I skip the blunder check.".into());
        }
        let mut best = cands[chosen.min(cands.len() - 1)].mv;
        // Concrete value of the move finally chosen, once checked by search.
        let mut best_val: Option<i32> = None;

        // Stage 6b: verification by real search (strong settings only).
        if p.verify_depth > 0 {
            // Club players' endgame technique lags their middlegame play: below
            // master strength, concrete checking is somewhat looser there.
            let eg = ((12 - crate::eval::game_phase(b)) as f32 / 8.0).clamp(0.0, 1.0) * (1.0 - p.engine_weight);
            let depth = ((p.verify_depth as f32) * (1.0 - 0.3 * eg) * hurry).round().max(1.0) as u32;
            let tolerance = p.verify_tolerance_cp * (1.0 + 0.8 * eg) * self.sloppy / (hurry * effort);
            let moves: Vec<Move> = cands.iter().map(|c| c.mv).collect();
            let budget = 20_000u64 * depth as u64 * depth as u64;
            let scored = self.searcher.score_moves(b, &moves, depth, Some(budget), limits.movetime_ms.map(|t| t * 35 / 100));
            let top = scored.iter().map(|x| x.1).max().unwrap_or(0);
            let ok: Vec<Move> = scored.iter().filter(|x| (x.1 as f32) >= top as f32 - tolerance).map(|x| x.0).collect();
            if !ok.contains(&best) {
                let pick = cands.iter().find(|c| ok.contains(&c.mv)).map(|c| c.mv).unwrap_or(best);
                trace.push(format!(
                    "Checking concretely, {} does not work; I prefer {}.",
                    san(b, best),
                    san(b, pick)
                ));
                best = pick;
            }
            best_val = scored.iter().find(|x| x.0 == best).map(|x| x.1);
        }

        // Stage 6c: top settings: let a full search challenge the human choice.
        if p.engine_weight > 0.0 {
            let lim = Limits {
                depth: limits.depth.or(Some((6.0 + 14.0 * p.engine_weight) as u32)),
                movetime_ms: limits.movetime_ms.map(|t| t * 45 / 100),
                nodes: limits.nodes,
                root_moves: None,
            };
            self.searcher.silent = true;
            let full = self.searcher.search(b, lim);
            self.searcher.silent = false;
            // Compare the human choice with the engine's at the same depth and by the same method.
            let pair = if full.best != best {
                self.searcher.score_moves(b, &[best, full.best], full.depth.saturating_sub(1).max(1), Some(2 * full.nodes.max(10_000)), limits.movetime_ms.map(|t| t / 5))
            } else {
                vec![]
            };
            let (my_score, engine_score) = match pair.as_slice() {
                [a, e] => (a.1, e.1),
                _ => (full.score, full.score),
            };
            if (my_score as f32) < engine_score as f32 - p.style_margin_cp && full.best != best {
                trace.push(format!(
                    "Deeper calculation ({} plies) finds {} ({}) better than {} ({}).",
                    full.depth,
                    san(b, full.best),
                    search::score_string(full.score),
                    san(b, best),
                    search::score_string(my_score)
                ));
                best = full.best;
                best_val = Some(engine_score);
            } else {
                best_val = Some(my_score);
                trace.push(format!("Deeper calculation confirms {}.", san(b, best)));
            }
        }

        // Stage 7: decision and simulated think time.
        let think_ms = self.think_time(b, best, &cands, percepts.len());
        trace.push(format!("Play {} after about {:.1} s.", san(b, best), think_ms as f32 / 1000.0));
        // Resignation: real Lichess players resign when clearly lost (median
        // about -12 pawns, a quarter by -8); many play on to mate. Per-move
        // hazard on how lost the position feels, calibrated on those games.
        // Judge by the move actually chosen: the first candidate may be a blunder the
        // check rejected (resigning at -3 because Rc8 hangs a knight, then playing Nge5).
        let hopeless = match best_val {
            Some(v) => (-v.max(-30000)).min(-feel_now + 300),
            None => (-feel_now).min(-cands.iter().find(|c| c.mv == best).unwrap_or(&cands[0]).value.max(-30000) + 300),
        };
        // About half of club players who are lost never resign that game: they play on
        // and get mated or flag. A per-game trait (fixed by the game's opening moves and
        // this side's colour) decides which kind of player this is today.
        let hist = self.searcher.game_history();
        let game_id = hist.get(10.min(hist.len().saturating_sub(1))).copied().unwrap_or(0) ^ (b.side as u64 * 0x9E37_79B9_7F4A_7C15);
        // Titled players almost never play on to mate (~10-15% of decided games end
        // in mate) and resign at a few pawns down, not twelve.
        let pro = ((self.persona.elo - 1800.0) / 1000.0).clamp(0.0, 1.0);
        let plays_on = (game_id.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) % 100 < (38.0 - 30.0 * pro) as u64;
        let mid = 900.0 - 200.0 * pro;
        let hazard = 0.35 / (1.0 + (-((hopeless as f32) - mid) / 150.0).exp());
        // Even stubborn players rarely wait to be mated with a bare king against
        // a queen or rook: twice as many HL games as human ones went that far.
        let has = |c: u8, kinds: &[u8]| b.sq.iter().any(|&x| x != 0 && piece_color(x) == c && kinds.contains(&piece_type(x)));
        let bare = !has(b.side, &[KNIGHT, BISHOP, ROOK, QUEEN]) && has(1 - b.side, &[ROOK, QUEEN]);
        let resign = hopeless > (500.0 - 100.0 * pro) as i32 && ((!plays_on && self.rng.chance(hazard)) || (bare && hopeless > 1200 && self.rng.chance(0.12)));
        if resign {
            trace.push(format!("This is hopeless ({}). I resign.", fmt_cp(-hopeless)));
        }
        Thought { best, think_ms, trace, candidates: cands, nodes: self.nodes, resign }
    }

    fn calculate_outer(&mut self, b: &Board, depth: u32, forgotten: &mut Vec<u8>, line: &mut Vec<Move>) -> i32 {
        self.calculate(b, depth, -INF, INF, forgotten, line, 1)
    }

    /// Simulated human think time, budgeted the way a strong player uses a clock.
    ///
    /// - An ordinary move gets an allotment: time left / moves still to play, plus most of the increment.
    /// - Obvious moves (recaptures, one candidate far ahead) take a small fraction of that.
    /// - Critical moves (close candidates, a busy position, the early middlegame) take a few
    ///   allotments, but never more than about a quarter of the remaining clock.
    /// - Move times are log-normal around those targets, as real move times are.
    /// Without a clock, `tc_scale` (1.0 = 5-minute game) sets the allotment.
    fn think_time(&mut self, b: &Board, best: Move, cands: &[Candidate], noticed: usize) -> u64 {
        let gap = if cands.len() >= 2 { (cands[0].value - cands[1].value).abs() as f32 } else { 500.0 };
        let move_no = b.fullmove as f32;
        let (allot, remaining) = match self.clock {
            Some((rem, inc)) => {
                let moves_left = (40.0 - 0.6 * move_no).max(15.0);
                // Club players budget optimistically and lose many games on time,
                // especially in bullet; strong players manage the clock better.
                let greed = 1.0 + 1.2 * ((2200.0 - self.persona.elo) / 1200.0).clamp(0.0, 1.0);
                // Strong players without increment keep a reserve for the ending:
                // titled 3+0 players average ~0.7 of base/40 per move and rarely flag.
                let greed = if inc <= 0.0 { greed - 0.35 * ((self.persona.elo - 2200.0) / 800.0).clamp(0.0, 1.0) } else { greed };
                (greed * rem.max(0.0) / moves_left + 0.8 * inc, rem.max(0.0))
            }
            None => (4.0 * self.tc_scale.clamp(0.1, 20.0), f32::INFINITY),
        };
        let recapture = self.last_capture_sq == Some(best.to) && b.sq[best.to as usize] != 0;
        let obvious = recapture || gap >= 300.0 || (gap >= 150.0 && self.rng.chance(0.5));
        let mut secs = if obvious {
            // Even an obvious move takes a human time to see the opponent's
            // move, confirm the idea and move the mouse: about a second in
            // blitz, a bit less in bullet, longer in slow games.
            (0.55 + 0.1 * allot) * (0.35 * self.rng.gauss()).exp()
        } else {
            let closeness = (1.0 - gap / 150.0).clamp(0.0, 1.0);
            let complexity = (noticed as f32 / 30.0).min(1.0);
            let phase = 0.6 * (-((move_no - 20.0) / 10.0).powi(2)).exp();
            let criticality = 0.4 + 1.4 * closeness + 0.6 * complexity + phase;
            // Opening familiarity: even out of book, early positions feel known
            // and go quickly; time use peaks in the early middlegame.
            let t = ((move_no - 6.0) / 10.0).clamp(0.0, 1.0);
            let familiar = 0.3 + 0.7 * t * t * (3.0 - 2.0 * t);
            // Human think times are heavy-tailed (titled blitz: CV ~1.3-1.4 per game);
            // sigma 0.9 with the mean kept by exp(-(0.9^2 - 0.65^2)/2).
            (0.82 * allot * criticality * familiar * (0.9 * self.rng.gauss()).exp()).min(6.0 * allot)
        };
        // Perception plus motor time: online players rarely move in under ~0.4 s
        // (premoves aside), except when the clock is nearly gone.
        // In a scramble a human still needs a few tenths of a second per move,
        // however little time is left; that is how flags fall.
        let floor = if remaining < 5.0 { 0.25 + 0.25 * self.rng.f32() } else { 0.4 + 0.1 * self.rng.f32() };
        secs = secs.min(0.25 * remaining).max(floor);
        // Premoves: in fast games an obvious reply (a recapture, the only sensible
        // move) is often queued before the opponent moves, so it takes a tenth of a
        // second or so. Top blitz players do this on ~18% of moves, more as the clock
        // runs down (measured from clocked online games).
        if self.clock.is_some() && obvious {
            // With an increment the clock is rarely tight, so premoves are much rarer
            // (titled 3+2: ~6% of moves under 0.5 s, vs ~18% at 3+0).
            let inc = self.clock.map_or(0.0, |c| c.1);
            let base_p = (0.75 - 0.06 * allot).clamp(0.05, 0.6) * if inc > 0.0 { 0.3 } else { 1.0 };
            let p_fast = base_p + if remaining < 20.0 { 0.2 } else { 0.0 };
            if self.rng.chance(p_fast) {
                let u = self.rng.f32();
                secs = 0.08 + 0.35 * u * u;
            }
        } else if self.clock.map_or(false, |c| c.1 <= 0.0) && gap >= 60.0 && self.rng.chance(0.08) {
            // Without increment players also bang out moves they half expected.
            secs = 0.25 + 0.25 * self.rng.f32();
        }
        (secs.min(600.0) * 1000.0) as u64
    }
}

pub fn fmt_cp(v: i32) -> String {
    if search::is_mate_score(v) {
        if v > 0 {
            "winning (mate)".into()
        } else {
            "lost (mate)".into()
        }
    } else {
        format!("{:+.1}", v as f32 / 100.0)
    }
}
