# Homo Ludens

A classical (non-neural) chess engine whose reasoning is a working model of a
human chess mind. At every rating from 100 to 3200 it perceives, recognises
patterns, calculates selectively, forgets, double-checks (or doesn't) and
errs the way a player of that rating does, and it explains every move.

This repository is **phase P0** of the research blueprint: the classical core,
the first version of the mind layers, and the benchmark harness.

## Build and run

```bash
cargo build --release
cargo test --release                      # 20 tests: perft, hashing, eval, search, mind

./target/release/homo_ludens              # UCI mode, for any chess GUI
./target/release/homo_ludens think 1500 "<FEN>"   # one full thought process
./target/release/homo_ludens selfplay 1200 1800 20 # persona vs persona match
./target/release/homo_ludens bench 9      # search speed
```

UCI options: `UCI_Elo` (100–3200), `UCI_LimitStrength` (human mode, default on),
`Seed` (reproducible personas), `ShowThoughts` (trace as `info string`), `Hash`.
In human mode each move also reports `info string thinktime <ms>`, the
simulated human think time.

### Example thought (Elo 1500, Black to move)

```
I notice 32 of 39 legal moves. Position feels -1.2 (piece placement favours them, ...).
  Qxf3 catches my eye: wins material (320).
Candidates: Bc5, Na6, Qe4.
Calculated 2 plies deep, 18 positions imagined.
  Na6: -2.3 (Na6 Bg5)
  Bc5: -5.2 (Bc5 dxc5) [first idea]
  Qe4: -12.4 (Qe4 Nxe4)
I skip the blunder check.
Play Na6 after about 3.2 s.
```

## How the code maps to the blueprint

| Blueprint stage | Code | What it does now (v0) |
| --- | --- | --- |
| Rating dial | `src/mind/persona.rs` | ~25 cognitive parameters, each a smooth monotonic curve over Elo. **Priors, not yet fitted.** |
| 1 Perception | `src/mind/perception.rs` | Which legal moves even come to mind: salience (captures, checks, threats, patterns), blind spots for backward and long moves |
| 2 Recognition | `perception.rs` (`PATTERNS`) | Hand-written v0 chunk library: fork, pin/skewer, back-rank. The learned library plugs in here in P1 |
| 3 Intuition | `src/mind/think.rs` | Softmax sampling of candidates; "can it just be taken?" check; first idea gets an Einstellung bonus |
| 4 Deliberation | `think.rs` (`calculate`) | Progressive deepening over *imagined* replies only; working-memory lapses (a moved piece is forgotten); rethinking when every idea disappoints |
| 5 Evaluation | `src/eval.rs` + `Mind::feel` | 9 named concepts; each persona knows only some of them, with stable per-position noise |
| 6 Verification | `src/search.rs` | Real alpha-beta (PVS, TT, null move, LMR, quiescence). Vetoes unsound candidates at 2300+; challenges the human choice at 2850+ |
| 7 Decision and timing | `think.rs` | Blunder check (probability rises with rating), simulated think time, thought trace |
| Harness | `harness/` | Move-matching accuracy by rating band, think-time correlation, Maia-2 style coherence test; grades any UCI engine |

## Measured status (Nov 2026)

| Check | Result |
| --- | --- |
| Move generation | Passes standard perft suites (start, Kiwipete, positions 3–5) |
| Incremental hashing | Verified against full recompute over every node of 4 perft trees |
| Search speed | ~640k nodes/s on one core (mailbox board, copy-make) |
| Strength is monotonic in Elo | Yes. Persona matches: 1200 beat 600 19–1; 1800 beat 1200 16–4; 2400 beat 1800 16.5–3.5; 3000 beat 2400 10–0 |
| Strength is calibrated | **No, not yet.** A nominal 600-Elo gap plays like ~500 Elo at the bottom and ~250 in the middle. Fitting the curves is P1–P2 work |
| Human move-matching | **Not yet measured on human games.** The included `data/sample_synthetic.pgn` is persona-vs-persona games for pipeline testing only; its numbers mean nothing about humans |
| 3000+ strength | **Not yet.** A 640k nps mailbox engine with this evaluation is likely in the 2200–2600 range against real engines; reaching 3000+ is P3 (bitboards, tuned eval) |

## Grading against real humans

```bash
pip install -r harness/requirements.txt
./harness/fetch_lichess.sh 2025-01 200000        # streams a month, keeps 200k games
python harness/benchmark.py --engine ./target/release/homo_ludens \
    --pgn data/lichess_2025-01_200000.pgn --max-positions 5000 --out reports/v0
```

Run the same command with Maia (via lc0) or Stockfish as `--engine` to get
baselines on identical positions. For absolute Elo calibration, play
`selfplay`-style gauntlets against Stockfish at fixed `UCI_Elo` settings
(e.g. with cutechess-cli).

## Next steps (P1, from the blueprint)

1. Run the harness on 5,000+ real Lichess positions and record the v0 baseline next to Maia.
2. Mine the chunk/template library from Lichess games and replace the hand-written patterns.
3. Fit persona curves by maximum likelihood on human moves, with the monotonicity constraint.
4. Fit the think-time model on real `%clk` data (target Spearman ≥ 0.5).
5. Calibrate the dial against Stockfish `UCI_Elo` gauntlets.
