//! The rating dial. Every cognitive parameter is a smooth function of Elo.
//!
//! The numbers here are PRIORS: plausible starting values drawn from the
//! cognitive literature and coaching experience. Phase P1 replaces them with
//! values fitted by maximum likelihood on human games (see `harness/`).
//! Keep every curve monotonic so strength rises smoothly with rating.

use crate::eval::N_CONCEPTS;

/// Elo anchor points shared by every curve below.
const ANCHORS: [f32; 8] = [100.0, 600.0, 1200.0, 1800.0, 2300.0, 2700.0, 2850.0, 3200.0];

fn curve(elo: f32, values: [f32; 8]) -> f32 {
    let e = elo.clamp(ANCHORS[0], ANCHORS[7]);
    for i in 0..7 {
        if e <= ANCHORS[i + 1] {
            let t = (e - ANCHORS[i]) / (ANCHORS[i + 1] - ANCHORS[i]);
            return values[i] + t * (values[i + 1] - values[i]);
        }
    }
    values[7]
}

/// Measured mapping from a public rating to the internal skill setting that
/// plays like it (harness/calibrate.py against real Lichess players: average
/// centipawn loss and blunder rate per rating band). Re-fit after any change
/// to the thinking stages.
const CALIB_LABEL: [f32; 7] = [100.0, 600.0, 1000.0, 1500.0, 2000.0, 2500.0, 3200.0];
const CALIB_INTERNAL: [f32; 7] = [600.0, 2340.0, 2690.0, 2780.0, 2890.0, 2970.0, 3200.0];

pub fn calibrated(label: f32) -> f32 {
    let l = label.clamp(CALIB_LABEL[0], CALIB_LABEL[6]);
    for i in 0..6 {
        if l <= CALIB_LABEL[i + 1] {
            let t = (l - CALIB_LABEL[i]) / (CALIB_LABEL[i + 1] - CALIB_LABEL[i]);
            return CALIB_INTERNAL[i] + t * (CALIB_INTERNAL[i + 1] - CALIB_INTERNAL[i]);
        }
    }
    CALIB_INTERNAL[6]
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Elo at which a player uses each evaluation concept at half weight.
/// Order matches `eval::Concept`.
const CONCEPT_MIDPOINT: [f32; N_CONCEPTS] = [
    -400.0, // material: everyone counts material
    900.0,  // piece placement
    1500.0, // mobility
    1600.0, // pawn structure
    1300.0, // passed pawns
    1200.0, // king safety
    1400.0, // rooks on open files
    1800.0, // bishop pair
    1500.0, // king activity in the endgame
];

#[derive(Clone, Debug)]
pub struct Persona {
    pub elo: f32,
    /// Internal (uncalibrated) skill setting the curves were read at.
    pub skill: f32,

    // --- Perception (stage 1)
    /// Chance to notice one of your own capture opportunities.
    pub p_see_capture: f32,
    /// Chance to notice that the opponent threatens one of your pieces.
    pub p_see_threat: f32,
    /// Extra chance to overlook a backward move (yours or the opponent's).
    pub p_blind_backward: f32,
    /// Extra chance to overlook a long slide (4+ squares) along a line.
    pub p_blind_long: f32,
    /// Chance to overlook a quiet (non-forcing) opponent reply while calculating.
    pub p_blind_quiet: f32,

    // --- Recognition and intuition (stages 2-3)
    /// Shifts how readily patterns are recognised (added to the player's Elo).
    pub pattern_skill: f32,
    pub max_candidates: usize,
    /// Softmax temperature (centipawns) when intuition ranks moves.
    pub intuition_temp: f32,
    /// Extra attraction of checks and captures, independent of their value.
    pub forcing_attraction: f32,

    // --- Working memory and deliberation (stage 4)
    pub calc_depth: u32,
    pub opp_replies: usize,
    pub node_budget: u32,
    /// Per ply of calculation: chance to forget where a moved piece now stands.
    pub p_forget: f32,
    /// Bonus given to the first idea that comes to mind (Einstellung).
    pub einstellung_cp: f32,
    /// Stop deliberating once a move is within this margin of the position's feel.
    pub satisfice_cp: f32,
    /// When every candidate disappoints, chance to go back and look for new ideas.
    pub p_rethink: f32,

    // --- Evaluation (stage 5)
    pub knowledge: [f32; N_CONCEPTS],
    pub eval_noise_cp: f32,

    // --- Safety and verification (stage 6)
    pub p_blunder_check: f32,
    /// Alpha-beta depth used to veto unsound candidates (0 = none).
    pub verify_depth: u32,
    /// Veto a candidate that verification says loses more than this.
    pub verify_tolerance_cp: f32,
    /// At the top settings: how far a human-preferred move may trail the engine's best.
    pub style_margin_cp: f32,
    /// Weight of the engine's own move in the top settings (0 = purely human candidates).
    pub engine_weight: f32,
}

impl Persona {
    pub fn from_elo(elo: f32) -> Persona {
        let label = elo.clamp(100.0, 3200.0);
        Self::from_internal(calibrated(label), label)
    }

    /// Persona at an uncalibrated internal skill setting, reporting `label` as its rating.
    pub fn from_internal(elo: f32, label: f32) -> Persona {
        let e = elo.clamp(ANCHORS[0], ANCHORS[7]);
        let mut knowledge = [0.0; N_CONCEPTS];
        for (k, mid) in knowledge.iter_mut().zip(CONCEPT_MIDPOINT.iter()) {
            *k = sigmoid((e - mid) / 250.0);
        }
        Persona {
            elo: label,
            skill: e,
            p_see_capture: curve(e, [0.55, 0.80, 0.93, 0.98, 0.995, 1.0, 1.0, 1.0]),
            p_see_threat: curve(e, [0.20, 0.50, 0.78, 0.92, 0.97, 0.99, 0.995, 1.0]),
            p_blind_backward: curve(e, [0.60, 0.45, 0.30, 0.15, 0.07, 0.03, 0.02, 0.0]),
            p_blind_long: curve(e, [0.50, 0.35, 0.22, 0.12, 0.06, 0.03, 0.02, 0.0]),
            p_blind_quiet: curve(e, [0.80, 0.60, 0.40, 0.22, 0.10, 0.05, 0.03, 0.0]),
            pattern_skill: 0.0,
            max_candidates: curve(e, [1.0, 2.0, 3.0, 3.0, 4.0, 5.0, 5.0, 6.0]).round() as usize,
            intuition_temp: curve(e, [220.0, 120.0, 70.0, 40.0, 25.0, 15.0, 10.0, 5.0]),
            forcing_attraction: curve(e, [80.0, 50.0, 25.0, 10.0, 5.0, 0.0, 0.0, 0.0]),
            calc_depth: curve(e, [0.0, 1.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0]).round() as u32,
            opp_replies: curve(e, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]).round() as usize,
            node_budget: curve(e, [5.0, 25.0, 100.0, 400.0, 2000.0, 8000.0, 20000.0, 50000.0]) as u32,
            p_forget: curve(e, [0.50, 0.35, 0.20, 0.12, 0.06, 0.03, 0.02, 0.0]),
            einstellung_cp: curve(e, [0.0, 20.0, 40.0, 50.0, 40.0, 30.0, 20.0, 5.0]),
            satisfice_cp: curve(e, [300.0, 200.0, 120.0, 70.0, 40.0, 20.0, 10.0, 0.0]),
            p_rethink: curve(e, [0.05, 0.25, 0.55, 0.80, 0.92, 0.97, 0.99, 1.0]),
            knowledge,
            eval_noise_cp: curve(e, [250.0, 150.0, 90.0, 55.0, 35.0, 22.0, 15.0, 5.0]),
            p_blunder_check: curve(e, [0.0, 0.25, 0.55, 0.80, 0.92, 0.97, 0.99, 1.0]),
            verify_depth: curve(e, [0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 6.0, 14.0]).round() as u32,
            verify_tolerance_cp: curve(e, [9999.0, 9999.0, 9999.0, 9999.0, 250.0, 120.0, 50.0, 10.0]),
            style_margin_cp: curve(e, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 200.0, 8.0]),
            engine_weight: curve(e, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        }
    }

    /// Probability that this player recognises a pattern typically learned at `pattern_elo`.
    pub fn recognises(&self, pattern_elo: f32) -> f32 {
        sigmoid((self.skill + self.pattern_skill - pattern_elo) / 200.0)
    }
}

/// Small deterministic RNG (xorshift64*), so a persona's "mood" is reproducible
/// for a given position and seed.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x2545_F491_4F6C_DD1D | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
    /// Roughly normal (sum of uniforms), mean 0, standard deviation 1.
    pub fn gauss(&mut self) -> f32 {
        (0..6).map(|_| self.f32()).sum::<f32>() * std::f32::consts::SQRT_2 - 6.0 * std::f32::consts::FRAC_1_SQRT_2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_are_monotonic() {
        let mut prev = Persona::from_elo(100.0);
        let mut e = 150.0;
        while e <= 3200.0 {
            let p = Persona::from_elo(e);
            assert!(p.p_see_threat >= prev.p_see_threat);
            assert!(p.p_blind_backward <= prev.p_blind_backward);
            assert!(p.calc_depth >= prev.calc_depth);
            assert!(p.eval_noise_cp <= prev.eval_noise_cp);
            assert!(p.p_blunder_check >= prev.p_blunder_check);
            for i in 0..N_CONCEPTS {
                assert!(p.knowledge[i] >= prev.knowledge[i]);
            }
            prev = p;
            e += 50.0;
        }
    }

    #[test]
    fn gauss_is_centered() {
        let mut r = Rng::new(7);
        let n = 20_000;
        let xs: Vec<f32> = (0..n).map(|_| r.gauss()).collect();
        let mean = xs.iter().sum::<f32>() / n as f32;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f32>() / n as f32;
        assert!(mean.abs() < 0.05, "mean {mean}");
        assert!((var.sqrt() - 1.0).abs() < 0.08, "sd {}", var.sqrt());
    }
}
