# Anti-cheat grade

Player-games: {'humans': 638, 'homo_ludens': 621}

## Detector (AUC, 0.50 = indistinguishable from humans)

| Bot | Logistic | Boosted trees | Boosted, without game length |
| --- | ---: | ---: | ---: |
| homo_ludens | 0.67 | 0.71 | 0.68 |

## homo_ludens

Lichess flags, humans vs homo_ludens: suspicious error rate 4% vs 2%; always has advantage 30% vs 29%

Time flags (bot only; no human clocks in this data): median move-time CV 0.99; 'highly consistent' 0%; 'no fast moves' 80% of games

| Signal | Humans | Bot | Z |
| --- | ---: | ---: | ---: |
| Average centipawn loss | 81.9 | 77.2 | -2.0 |
| Lichess accuracy % | 72.9 | 75.8 | +3.9 |
| Engine top-move match | 0.346 | 0.317 | -3.0 |
| Regan scaled error | 0.364 | 0.368 | +0.4 |
| Inaccuracy rate | 0.0974 | 0.116 | +4.6 |
| Mistake rate | 0.0481 | 0.0542 | +2.1 |
| Blunder rate | 0.087 | 0.0889 | +0.5 |
| Top-move match, only-move positions | 0.717 | 0.751 | +2.0 |
| Top-move match, several good moves | 0.314 | 0.28 | -5.7 |
| ACPL when worse by 1+ pawn | 84.2 | 78.2 | -1.3 |
| Never worse than -1 (Lichess flag) | 0.304 | 0.291 | -0.5 |
| ACPL, moves 5-10 | 60.2 | 59.1 | -0.3 |
| ACPL, middlegame | 93.6 | 90.3 | -1.0 |
| ACPL, endgame | 69.8 | 55.8 | -2.4 |
| Game length (plies) | 72.8 | 79 | +3.7 |

Strongest giveaways (single-signal AUC): Top-move match, several good moves 0.59, Inaccuracy rate 0.58, ACPL, moves 5-10 0.57, Game length (plies) 0.56, Regan scaled error 0.56
