# Running HL as a Lichess bot

The bot is [lichess-bot](https://github.com/lichess-bot-devs/lichess-bot) (pinned in the Dockerfile) with HL as its engine.

How it plays:
- Each game is played at one of six fixed levels, 800, 1200, 1600, 2000, 2400 or 2800: whichever is closest to the opponent's Lichess rating. To pin one level for every game, set `HL_BOT_ELO`.
- The time control is passed to HL so it budgets the clock like a human would.
- Each move is computed with the same budget the anti-cheat evals used (about 0.5 s). The move is then held back until HL's simulated human think time is up, so it never moves instantly.
- Bullet, blitz and rapid; standard chess only; up to 2 games at once.

## Running it

```bash
docker build -f bot/Dockerfile -t hl-bot .
docker run -e LICHESS_BOT_TOKEN=<token> hl-bot
```
