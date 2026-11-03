"""Minimal UCI client that keeps every `info string` line.

python-chess's engine module only keeps the last info string, but the harness
needs Homo Ludens's `thinktime` line and its thought trace, so this talks to
the engine process directly. Works with any UCI engine (Stockfish, lc0 + Maia).
"""

from __future__ import annotations

import subprocess
from dataclasses import dataclass, field


@dataclass
class EngineReply:
    move: str
    think_ms: int | None = None
    trace: list[str] = field(default_factory=list)


class UciEngine:
    def __init__(self, cmd: list[str], options: dict[str, str] | None = None):
        self.proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
        )
        self._send("uci")
        self.options: set[str] = set()
        for line in self._read_until("uciok"):
            if line.startswith("option name "):
                name = line[len("option name "):].split(" type ")[0]
                self.options.add(name.lower())
        for k, v in (options or {}).items():
            self.set_option(k, v)
        self.ready()

    def _send(self, s: str) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(s + "\n")
        self.proc.stdin.flush()

    def _read_until(self, prefix: str) -> list[str]:
        assert self.proc.stdout is not None
        lines = []
        while True:
            line = self.proc.stdout.readline()
            if not line:
                raise RuntimeError("engine exited unexpectedly")
            line = line.rstrip("\n")
            lines.append(line)
            if line.startswith(prefix):
                return lines

    def ready(self) -> None:
        self._send("isready")
        self._read_until("readyok")

    def set_option(self, name: str, value) -> None:
        if name.lower() in self.options:
            self._send(f"setoption name {name} value {value}")

    def new_game(self) -> None:
        self._send("ucinewgame")
        self.ready()

    def play(self, fen: str, moves: list[str] | None = None, go: str = "movetime 100") -> EngineReply:
        pos = f"position fen {fen}"
        if moves:
            pos += " moves " + " ".join(moves)
        self._send(pos)
        self._send(f"go {go}")
        reply = EngineReply(move="0000")
        for line in self._read_until("bestmove"):
            if line.startswith("info string "):
                text = line[len("info string "):]
                if text.startswith("thinktime "):
                    reply.think_ms = int(text.split()[1])
                else:
                    reply.trace.append(text)
            elif line.startswith("bestmove"):
                reply.move = line.split()[1]
        return reply

    def close(self) -> None:
        try:
            self._send("quit")
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
