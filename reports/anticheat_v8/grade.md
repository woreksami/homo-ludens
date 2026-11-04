# Anti-cheat grade

Player-games: {'humans': 638, 'homo_ludens': 618}

## Detector (AUC, 0.50 = indistinguishable from humans)

| Bot | Logistic | Boosted trees | Boosted, without game length |
| --- | ---: | ---: | ---: |
| homo_ludens | 0.68 | 0.73 | 0.69 |

## homo_ludens

Lichess flags, humans vs homo_ludens: suspicious error rate 3% vs 1%; always has advantage 30% vs 28%

Time flags (bot only; no human clocks in this data): median move-time CV 0.88; 'highly consistent' 0%; 'no fast moves' 83% of games

| Signal | Humans | Bot | Z |
| --- | ---: | ---: | ---: |
| Average centipawn loss | 83 | 79.9 | -1.3 |
| Lichess accuracy % | 73.2 | 76.3 | +4.3 |
| Engine top-move match | 0.341 | 0.325 | -1.7 |
| Regan scaled error | 0.359 | 0.352 | -0.6 |
| Inaccuracy rate | 0.102 | 0.121 | +4.4 |
| Mistake rate | 0.0492 | 0.062 | +4.5 |
| Blunder rate | 0.0876 | 0.0862 | -0.3 |
| Top-move match, only-move positions | 0.722 | 0.742 | +1.2 |
| Top-move match, several good moves | 0.306 | 0.28 | -4.3 |
| ACPL when worse by 1+ pawn | 87.3 | 81.7 | -1.3 |
| Never worse than -1 (Lichess flag) | 0.304 | 0.282 | -0.9 |
| ACPL, moves 5-10 | 60 | 58.9 | -0.3 |
| ACPL, middlegame | 95.3 | 92.1 | -1.0 |
| ACPL, endgame | 68.9 | 56.2 | -2.4 |
| Game length (plies) | 72.5 | 86.8 | +7.5 |

Strongest giveaways (single-signal AUC): Game length (plies) 0.60, Mistake rate 0.59, Inaccuracy rate 0.58, Top-move match, several good moves 0.58, ACPL, moves 5-10 0.56
