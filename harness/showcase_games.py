"""Record Homo Ludens self-play games with full thought traces and clocks, as JSON for the viewer."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import chess

sys.path.insert(0, str(Path(__file__).resolve().parent))
from uci_client import UciEngine  # noqa: E402


def play(hl: str, elo: int, base: float, inc: float, seed: int) -> dict:
    e = UciEngine([hl], {"UCI_LimitStrength": "true", "ShowThoughts": "true", "UCI_Elo": elo, "SimTimeControl": f"{int(base)}+{int(inc)}"})
    b = chess.Board()
    clocks = {chess.WHITE: base, chess.BLACK: base}
    moves, plies = [], []
    result, termination = "*", "Normal"
    while not b.is_game_over(claim_draw=True) and b.ply() < 220:
        e.set_option("Seed", seed * 1000 + b.ply())
        go = f"movetime 200 wtime {int(clocks[chess.WHITE]*1000)} btime {int(clocks[chess.BLACK]*1000)} winc {int(inc*1000)} binc {int(inc*1000)}"
        r = e.play(chess.STARTING_FEN, moves, go=go)
        secs = (r.think_ms or 0) / 1000
        trace = [t for t in r.trace if t != "resign"]
        if "resign" in r.trace:
            result = "0-1" if b.turn == chess.WHITE else "1-0"
            plies.append({"resign": True, "think": round(secs, 2), "trace": trace})
            break
        clocks[b.turn] -= secs
        if clocks[b.turn] <= 0:
            result, termination = ("0-1" if b.turn == chess.WHITE else "1-0"), "Time forfeit"
            plies.append({"flag": True, "think": round(secs + clocks[b.turn], 2), "trace": trace})
            break
        clocks[b.turn] += inc
        m = chess.Move.from_uci(r.move)
        san = b.san(m)
        b.push(m)
        moves.append(r.move)
        plies.append({"uci": r.move, "san": san, "fen": b.fen(), "think": round(secs, 2), "trace": trace,
                      "clock": round(clocks[not b.turn], 1)})
    if result == "*":
        result = b.result(claim_draw=True)
    e.close()
    return {"white": f"Homo Ludens {elo}", "black": f"Homo Ludens {elo}", "elo": elo,
            "tc": f"{int(base // 60)}+{int(inc)}", "base": base, "result": result, "termination": termination, "plies": plies}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--hl", required=True)
    ap.add_argument("--games", default="800:180:0,1400:180:2,2000:300:0,2600:300:3")
    ap.add_argument("--out", type=Path, default=Path("reports/showcase/games.json"))
    args = ap.parse_args()
    games = []
    for i, spec in enumerate(args.games.split(",")):
        elo, base, inc = (int(x) for x in spec.split(":"))
        games.append(play(args.hl, elo, base, inc, seed=17 + i))
        print(f"game {i+1}: {games[-1]['white']} {games[-1]['tc']} {games[-1]['result']} {len(games[-1]['plies'])} plies", file=sys.stderr)
        args.out.write_text(json.dumps(games))


if __name__ == "__main__":
    main()
