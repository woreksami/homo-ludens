# Homo Ludens on Lichess: HL built from this repo + lichess-bot as the bridge.
# Build from the repo root:  docker build -f bot/Dockerfile .
FROM rust:1-slim AS build
WORKDIR /hl
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo build --release --bin homo_ludens

FROM python:3.12-slim
ARG LICHESS_BOT_REF=baa2b7e78320b99125a91c2b1805bb7a5c4fb6ea
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && git init -q /lichess-bot && cd /lichess-bot \
 && git fetch -q --depth 1 https://github.com/lichess-bot-devs/lichess-bot "$LICHESS_BOT_REF" \
 && git checkout -q FETCH_HEAD \
 && pip install --no-cache-dir -r requirements.txt
WORKDIR /lichess-bot
COPY --from=build /hl/target/release/homo_ludens engines/homo_ludens
COPY data/book.tsv engines/data/book.tsv
COPY bot/config.yml config.yml
COPY bot/extra_game_handlers.py extra_game_handlers.py
ENV PYTHONUNBUFFERED=1
# -u turns the account into a BOT account the first time (lichess-bot skips it if it already is one)
CMD ["python3", "lichess-bot.py", "-u", "--disable_auto_logging"]
