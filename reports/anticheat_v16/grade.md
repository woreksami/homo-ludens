# Anti-cheat grade

Player-games: {'humans': 638, 'homo_ludens': 609}

## Detector (AUC, 0.50 = indistinguishable from humans)

| Bot | Logistic | Boosted trees | Boosted, without game length |
| --- | ---: | ---: | ---: |
| homo_ludens | 0.67 | 0.69 | 0.66 |

## homo_ludens

Lichess flags, humans vs homo_ludens: suspicious error rate 4% vs 1%; always has advantage 30% vs 29%

Time flags (bot only; no human clocks in this data): median move-time CV 1.00; 'highly consistent' 0%; 'no fast moves' 80% of games

| Signal | Humans | Bot | Z |
| --- | ---: | ---: | ---: |
| Average centipawn loss | 81.5 | 78.9 | -1.1 |
| Lichess accuracy % | 73 | 75.3 | +2.9 |
| Engine top-move match | 0.347 | 0.32 | -2.9 |
| Regan scaled error | 0.362 | 0.371 | +0.6 |
| Inaccuracy rate | 0.0974 | 0.115 | +4.5 |
| Mistake rate | 0.0481 | 0.0543 | +2.1 |
| Blunder rate | 0.0868 | 0.0913 | +1.1 |
| Top-move match, only-move positions | 0.716 | 0.71 | -0.4 |
| Top-move match, several good moves | 0.315 | 0.281 | -5.7 |
| ACPL when worse by 1+ pawn | 84 | 79 | -1.0 |
| Never worse than -1 (Lichess flag) | 0.305 | 0.294 | -0.4 |
| ACPL, moves 5-10 | 60.2 | 58.3 | -0.6 |
| ACPL, middlegame | 93.7 | 93.1 | -0.2 |
| ACPL, endgame | 67.7 | 50.1 | -3.1 |
| Game length (plies) | 72.5 | 80.6 | +4.9 |

Strongest giveaways (single-signal AUC): Top-move match, several good moves 0.60, Game length (plies) 0.58, Inaccuracy rate 0.58, ACPL, moves 5-10 0.57, Engine top-move match 0.56
