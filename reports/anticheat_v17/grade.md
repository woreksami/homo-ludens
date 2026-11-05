# Anti-cheat grade

Player-games: {'humans': 638, 'homo_ludens': 620}

## Detector (AUC, 0.50 = indistinguishable from humans)

| Bot | Logistic | Boosted trees | Boosted, without game length |
| --- | ---: | ---: | ---: |
| homo_ludens | 0.67 | 0.71 | 0.67 |

## homo_ludens

Lichess flags, humans vs homo_ludens: suspicious error rate 4% vs 2%; always has advantage 30% vs 26%

Time flags (bot only; no human clocks in this data): median move-time CV 1.00; 'highly consistent' 0%; 'no fast moves' 81% of games

| Signal | Humans | Bot | Z |
| --- | ---: | ---: | ---: |
| Average centipawn loss | 81.8 | 78.3 | -1.5 |
| Lichess accuracy % | 72.9 | 75.4 | +3.3 |
| Engine top-move match | 0.347 | 0.315 | -3.5 |
| Regan scaled error | 0.363 | 0.369 | +0.5 |
| Inaccuracy rate | 0.0976 | 0.112 | +3.7 |
| Mistake rate | 0.0483 | 0.0563 | +2.7 |
| Blunder rate | 0.0871 | 0.0928 | +1.4 |
| Top-move match, only-move positions | 0.717 | 0.72 | +0.2 |
| Top-move match, several good moves | 0.314 | 0.284 | -5.0 |
| ACPL when worse by 1+ pawn | 84.3 | 79.7 | -0.9 |
| Never worse than -1 (Lichess flag) | 0.302 | 0.256 | -1.8 |
| ACPL, moves 5-10 | 60.2 | 60.7 | +0.2 |
| ACPL, middlegame | 94 | 90.6 | -1.0 |
| ACPL, endgame | 68.9 | 53.4 | -2.8 |
| Game length (plies) | 72.5 | 83.1 | +6.0 |

Strongest giveaways (single-signal AUC): Game length (plies) 0.60, Top-move match, several good moves 0.59, ACPL, moves 5-10 0.58, Engine top-move match 0.56, Inaccuracy rate 0.56
