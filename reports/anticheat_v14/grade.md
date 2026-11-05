# Anti-cheat grade

Player-games: {'humans': 638, 'homo_ludens': 617}

## Detector (AUC, 0.50 = indistinguishable from humans)

| Bot | Logistic | Boosted trees | Boosted, without game length |
| --- | ---: | ---: | ---: |
| homo_ludens | 0.65 | 0.70 | 0.67 |

## homo_ludens

Lichess flags, humans vs homo_ludens: suspicious error rate 4% vs 1%; always has advantage 30% vs 29%

Time flags (bot only; no human clocks in this data): median move-time CV 1.01; 'highly consistent' 0%; 'no fast moves' 80% of games

| Signal | Humans | Bot | Z |
| --- | ---: | ---: | ---: |
| Average centipawn loss | 81.8 | 76.7 | -2.1 |
| Lichess accuracy % | 72.9 | 75.8 | +3.9 |
| Engine top-move match | 0.347 | 0.319 | -2.9 |
| Regan scaled error | 0.363 | 0.359 | -0.3 |
| Inaccuracy rate | 0.0979 | 0.113 | +3.8 |
| Mistake rate | 0.048 | 0.0564 | +2.9 |
| Blunder rate | 0.0871 | 0.0878 | +0.2 |
| Top-move match, only-move positions | 0.718 | 0.731 | +0.8 |
| Top-move match, several good moves | 0.315 | 0.284 | -5.2 |
| ACPL when worse by 1+ pawn | 84.3 | 78.8 | -1.1 |
| Never worse than -1 (Lichess flag) | 0.303 | 0.293 | -0.4 |
| ACPL, moves 5-10 | 60 | 58.9 | -0.3 |
| ACPL, middlegame | 94.2 | 88.1 | -1.8 |
| ACPL, endgame | 68.9 | 53.7 | -2.7 |
| Game length (plies) | 72.6 | 80.6 | +4.5 |

Strongest giveaways (single-signal AUC): Top-move match, several good moves 0.59, ACPL, moves 5-10 0.57, Game length (plies) 0.57, Inaccuracy rate 0.56, Mistake rate 0.56
