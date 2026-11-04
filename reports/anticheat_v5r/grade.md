# Anti-cheat grade

Player-games: {'humans': 638, 'homo_ludens': 613}

## Detector (AUC, 0.50 = indistinguishable from humans)

| Bot | Logistic | Boosted trees | Boosted, without game length |
| --- | ---: | ---: | ---: |
| homo_ludens | 0.71 | 0.76 | 0.73 |

## homo_ludens

Lichess flags, humans vs homo_ludens: suspicious error rate 3% vs 1%; always has advantage 31% vs 24%

Time flags (bot only; no human clocks in this data): median move-time CV 0.89; 'highly consistent' 0%; 'no fast moves' 83% of games

| Signal | Humans | Bot | Z |
| --- | ---: | ---: | ---: |
| Average centipawn loss | 82.3 | 85.2 | +1.2 |
| Lichess accuracy % | 73.4 | 74.5 | +1.5 |
| Engine top-move match | 0.342 | 0.311 | -3.3 |
| Regan scaled error | 0.357 | 0.389 | +2.5 |
| Inaccuracy rate | 0.102 | 0.128 | +6.3 |
| Mistake rate | 0.0492 | 0.0683 | +6.3 |
| Blunder rate | 0.0859 | 0.0995 | +3.3 |
| Top-move match, only-move positions | 0.724 | 0.749 | +1.5 |
| Top-move match, several good moves | 0.306 | 0.268 | -6.2 |
| ACPL when worse by 1+ pawn | 86.8 | 83.1 | -0.8 |
| Never worse than -1 (Lichess flag) | 0.309 | 0.243 | -2.6 |
| ACPL, moves 5-10 | 58 | 66.8 | +2.8 |
| ACPL, middlegame | 95 | 100 | +1.6 |
| ACPL, endgame | 68.8 | 58.5 | -2.0 |
| Game length (plies) | 72.9 | 82.6 | +5.3 |

Strongest giveaways (single-signal AUC): ACPL, moves 5-10 0.63, Inaccuracy rate 0.61, Mistake rate 0.61, Top-move match, several good moves 0.60, Regan scaled error 0.59
