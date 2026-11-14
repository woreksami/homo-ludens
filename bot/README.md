# Running HL as a Lichess bot

The bot is [lichess-bot](https://github.com/lichess-bot-devs/lichess-bot) (pinned in the Dockerfile) with HL as its engine.

How it plays:
- Each game is played at one of six fixed levels, 800, 1200, 1600, 2000, 2400 or 2800: whichever is closest to the opponent's Lichess rating. To pin one level for every game, set `HL_BOT_ELO`.
- The time control is passed to HL so it budgets the clock like a human would.
- Each move is computed with the same budget the anti-cheat evals used (about 0.5 s). The move is then held back until HL's simulated human think time is up, so it never moves instantly.
- Bullet, blitz and rapid; standard chess only; up to 2 games at once.

## Deploy on fly.io

Quickest: run `bot/deploy.sh` from the repo root. It logs you in to fly, creates the app, asks for the token (hidden), checks it with Lichess and deploys. The manual steps are below.

You need a fly.io account, [flyctl](https://fly.io/docs/flyctl/install/) and a Lichess API token for the bot account with the `bot:play` scope (create it at https://lichess.org/account/oauth/token/create while logged in as the bot).

```bash
fly launch --no-deploy --copy-config      # once; keeps fly.toml, pick your own app name if asked
fly secrets set LICHESS_BOT_TOKEN=<token>
fly deploy --ha=false                     # one machine is enough
fly logs                                  # watch it connect and accept challenges
```

On its first start the bot upgrades the account to a BOT account. This can't be undone, and it only works on an account that has never played a game.

Optional: `fly secrets set HL_BOT_ELO=2000` makes it play every game at 2000.

## Run it locally instead

```bash
docker build -f bot/Dockerfile -t hl-bot .
docker run -e LICHESS_BOT_TOKEN=<token> hl-bot
```
