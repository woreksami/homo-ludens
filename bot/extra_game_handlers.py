"""HL-specific hooks for lichess-bot (copied over lichess-bot's own file in the Docker image)."""
import os

from lib import model
from lib.lichess_types import OPTIONS_TYPE

# the strengths the bot plays at; each game uses the one closest to the opponent
LEVELS = [800, 1200, 1600, 2000, 2400, 2800]


def pick_level(rating: int | None) -> int:
    fixed = os.environ.get("HL_BOT_ELO")
    if fixed:
        return int(fixed)
    return min(LEVELS, key=lambda lv: abs(lv - (rating or 1500)))


def game_specific_options(game: model.Game) -> OPTIONS_TYPE:
    base = game.clock_initial.total_seconds()
    inc = game.clock_increment.total_seconds()
    return {
        "UCI_Elo": pick_level(game.opponent.rating),
        "SimTimeControl": f"{int(base)}+{int(inc)}",
    }


def is_supported_extra(challenge: model.Challenge) -> bool:  # noqa: ARG001
    return True
