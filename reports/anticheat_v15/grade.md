# Anti-cheat grade

Player-games: {'humans': 638, 'homo_ludens': 621}

## Detector (AUC, 0.50 = indistinguishable from humans)

| Bot | Logistic | Boosted trees | Boosted, without game length |
| --- | ---: | ---: | ---: |
| homo_ludens | 0.65 | 0.70 | 0.67 |

## homo_ludens

Lichess flags, humans vs homo_ludens: suspicious error rate 4% vs 0%; always has advantage 30% vs 29%

Time flags (bot only; no human clocks in this data): median move-time CV 1.00; 'highly consistent' 0%; 'no fast moves' 81% of games

| Signal | Humans | Bot | Z |
| --- | ---: | ---: | ---: |
| Average centipawn loss | 81.8 | 76.7 | -2.2 |
| Lichess accuracy % | 73 | 75.7 | +3.7 |
| Engine top-move match | 0.346 | 0.324 | -2.4 |
| Regan scaled error | 0.364 | 0.36 | -0.3 |
| Inaccuracy rate | 0.0977 | 0.121 | +5.8 |
| Mistake rate | 0.0482 | 0.0594 | +3.8 |
| Blunder rate | 0.087 | 0.0856 | -0.4 |
| Top-move match, only-move positions | 0.717 | 0.747 | +1.8 |
| Top-move match, several good moves | 0.314 | 0.282 | -5.3 |
| ACPL when worse by 1+ pawn | 84.5 | 76.6 | -1.6 |
| Never worse than -1 (Lichess flag) | 0.304 | 0.287 | -0.7 |
| ACPL, moves 5-10 | 60.5 | 59.4 | -0.4 |
| ACPL, middlegame | 93.9 | 87.3 | -2.0 |
| ACPL, endgame | 69 | 54.9 | -2.4 |
| Game length (plies) | 72.7 | 79.4 | +4.0 |

Strongest giveaways (single-signal AUC): Inaccuracy rate 0.60, Top-move match, several good moves 0.59, Mistake rate 0.57, ACPL, moves 5-10 0.57, Game length (plies) 0.57
