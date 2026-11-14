# Homo Ludens

Homo Ludens is a classical (non-neural) chess engine whose move choice is a
model of how a human player of a given rating thinks. Set the rating anywhere
from 100 to 3200. The engine then:

- notices some legal moves and misses others;
- calculates only the lines that come to mind;
- forgets things;
- double-checks, or skips the check;
- spends its clock;
- resigns or plays on;

all at rates fitted to real games at that rating.

The goal is measurable. A statistical detector trained to tell the engine's
games from human games at the same rating and opening should do no better than
chance (AUC 0.50). The project's target is AUC below 0.60 at every rating,
including at the top, where the engine has to play at grandmaster strength.

The engine is a research project. It is meant to play openly as a computer
(for example in chess.com computer tournaments). It is not meant for human-rated
play.

## Status (November 2026)

The detector's AUC for the current version, v47, on three test sets:

| Test set | Logistic | Boosted trees | Boosted, no game length | HL ACPL | Human ACPL |
| --- | ---: | ---: | ---: | ---: | ---: |
| Carlsen, chess.com 3+0, rated 2800-3200 | 0.60 | 0.62 | 0.62 | 26 | 32 |
| Titled Tuesday 3+2, rated 2200-2800 | 0.68 | 0.66 | 0.66 | 34 | 40 |

These two v47 results are with the Stockfish oracle checking 30% of moves (see
below). Without the oracle, v46 scores 0.69 / 0.72 / 0.69 and 0.68 / 0.68 / 0.68
on the same two sets.

The third test set is Lichess 2016-17, 1000-2200, mostly 10+0. There, a
like-for-like comparison of three engines replaying the same games gives:

| Engine | Logistic | Boosted trees | Boosted, no game length |
| --- | ---: | ---: | ---: |
| Homo Ludens (v39) | 0.65 | 0.67 | 0.61 |
| Maia-1, rating-matched | 0.74 | 0.77 | 0.73 |
| Stockfish, full strength | 0.95 | 0.93 | 0.90 |

Read these numbers with their noise in mind. Re-running an identical binary
moves the AUC by up to about ±0.07, because think times are not
deterministic. A single run below 0.60 is not yet a result; we need repeated
runs below 0.60.

Two signals still separate the engine from strong humans most reliably:

- **Defence:** the engine defends too accurately when a pawn or more down. Humans
  lose about 64 cp per move there; the engine loses about 38.
- **Small inaccuracies:** the engine makes slightly too many small
  inaccuracies relative to outright blunders.

Measured in Stockfish's centipawn loss, the engine plays at its set rating's
level. At the top setting with the oracle it plays somewhat above it.

## How a move is chosen

All of this lives in `src/mind/think.rs`, except where another file is named.

1. **Perception** (`perception.rs`). Decides which legal moves come to mind at
   all. Salience rises for captures, checks, threats and recognised patterns.
   Backward and long moves are often missed, and the miss rate falls with
   rating.
2. **Opening memory** (`book.rs`). Early positions are played from a book built
   from real games, split by rating band. Players at each rating play the
   openings people at that rating actually play. None of the test games are in
   the book.
3. **Intuition.** Candidate moves are sampled with a softmax over how the move
   feels. The first idea gets a persistence bonus (Einstellung).
4. **Calculation.** Selective deepening over imagined replies only, with
   working-memory lapses: a piece that moved earlier in the line can be
   forgotten.
5. **Evaluation** (`eval.rs`, `Mind::feel`). Nine named positional concepts. Each
   persona knows a rating-dependent subset of them, with stable per-position
   noise.
6. **Verification** (`search.rs`). From about 2300, a real alpha-beta search
   (PVS, transposition table, null move, LMR, quiescence) can veto a candidate
   that loses material.
7. **Deep check** (top settings only). A full search can overrule the human
   choice when its own move is clearly better. A sacrifice that the search shows
   gaining at least about 60 cp is played even if style would avoid it.
8. **Decision and time.** Blunder check (its probability rises with rating),
   resignation and simulated think time. Time is budgeted by the clock and by
   how critical and familiar the position feels, with the spread of think times
   real players show. Moves never come instantly; premoves take at least
   0.08 s.

Every move comes with a plain-language trace of this process.

Around 26 cognitive parameters set the rating (`persona.rs`). Each is a smooth
curve over Elo, fitted against real games band by band.

Example trace (Elo 1500):

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

### Optional Stockfish oracle

The engine's own search is weaker than a world-class player in sharp
positions. Given unlimited time, it matches Stockfish's top move on about 48%
of positions from Carlsen's games, against Carlsen's 57%. For that reason the
deep check (stage 7) can be handed to an external UCI engine:

```
HL_ORACLE=/path/to/stockfish  HL_ORACLE_P=0.3  HL_ORACLE_NODES=150000
```

`HL_ORACLE_P` is the share of moves the oracle checks. The rest of the move
choice is unchanged. Only the oracle's verdict at stage 7 is external.

When the oracle checked every move, the engine became too clean. At Carlsen's
level it blundered on 0.2% of moves against the humans' 2.7%, and the detector
scored 0.83-0.90. Adding random lapses fixed the per-move numbers, but the
games then ended about 30 plies early. Checking 30% of moves has worked best so
far.

Stockfish is GPL-licensed. Homo Ludens only calls it as a separate process;
distributing the two together brings GPL obligations.

## Build and run

```bash
cargo build --release
cargo test --release

./target/release/homo_ludens                        # UCI mode, for any GUI
./target/release/homo_ludens think 1500 "<FEN>"     # one move with its trace
./target/release/homo_ludens selfplay 1200 1800 20  # persona vs persona
./target/release/homo_ludens bench 9                # search speed
```

UCI options:

| Option | Meaning |
| --- | --- |
| `UCI_Elo` | 100-3200 |
| `UCI_LimitStrength` | Human mode, on by default. Off: plain full-strength search |
| `Seed` | Reproducible persona |
| `ShowThoughts` | Sends the trace as `info string` |
| `SimTimeControl` | Time control, e.g. `180+0`, for the time model |
| `RawSkill` | Uses the persona at the uncalibrated internal rating, skipping the rating-to-skill map |
| `BookFile` | Opening book path |
| `Hash` | Hash table size |

In human mode each move also reports `info string thinktime <ms>`, the
simulated human think time.

Research knobs are read from the environment, and all are neutral by default.
They are documented where they are read in `think.rs` and `persona.rs`.

A browser build (`src/wasm_api.rs`, `viewer/play/`) lets you play the engine
and watch its thoughts. To build it:

```bash
RUSTC_BOOTSTRAP=1 cargo build --release --lib --target wasm32-unknown-unknown -Zbuild-std=std,panic_abort
```

## Evaluation method

Code is in `harness/anticheat/`. One eval run:

```bash
HLMS=500 JOBS=2 SETS="c" harness/eval_both.sh v47 v46
```

This runs `make_games.py`, `analyse.py` and `grade.py` in turn. The report is
written to `reports/c_v47/grade.md`.

1. **Sample.** Draw human games from a test set, with both players in the same
   rating band.
2. **Replay.** For each human game, the engine replays the first 8 plies and
   then plays both sides itself. Each side plays at the rating of the human it
   replaces, under the same time control. Openings and ratings are therefore
   identical, and any difference comes from how moves are chosen.
3. **Analyse.** Every position, human and engine, is analysed by Stockfish at
   50k nodes.
4. **Grade.** For each player-game, compute 15 features:
   - ACPL, overall and by phase;
   - Lichess accuracy;
   - top-move match in critical and in ambiguous positions;
   - Regan-style scaled error;
   - inaccuracy, mistake and blunder rates;
   - ACPL when worse;
   - game length.

   Then train logistic regression and gradient-boosted trees to separate
   engine from human. Cross-validation keeps each human game and its replay in
   the same fold. The report gives each detector's AUC, each feature's AUC on
   its own, Z-scores per rating band and Lichess's own move-quality flags.
   Move-time features are reported separately, only for sets where the human
   games carry clocks.

`harness/quick_acc.py` is a quicker check. It compares the engine's move with
the human's on sampled positions, which is useful for tuning before a full run.
Single-position results can mislead, though. Errors concentrate in time
trouble and unbalanced positions, which a small sample under-represents. Every
change is judged on full games.

## Data

- `data/book.tsv`: the rating-banded opening book. Build it with
  `harness/build_book.py`; test games are excluded by site, players, date and
  round.
- `data/carlsen/chesscom.pgn`: Carlsen's public chess.com blitz games, used for
  testing at 2800-3200.
- `data/tt/tt2019.pgn`: Titled Tuesday 2019, used for testing at 2200-2800.
- `data/kaggle/`: Lichess games from 2016-17, from the Kaggle "Chess Game
  Dataset". Convert it with `harness/kaggle_to_pgn.py`. These games are used
  for testing at 1000-2200 and are kept out of git. Fresh Lichess months can be
  fetched with `harness/fetch_lichess.sh`.

Game data comes from public sources and belongs to them. Reports
(`reports/`) and binaries (`bin/`) are not versioned. The figures above come
from the grade reports of the named versions.

## History

Versions are tagged in commit messages (`v1` ... `v47`). Each commit message
records the change and the eval it was judged on.

The project started (`cad23ae`) as a classical core with hand-set rating curves.
At that point the engine matched 28.8% of human moves on Lichess 2013
positions, against Maia-1's 50.7% (`results/baseline_v0.md`). Since then the
work has been driven by the detector. Each change targets the feature that
gives the engine away most clearly in the latest grade report, and it stays
only if a full run does not get worse.
