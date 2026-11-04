"""Inline reports/showcase/games.json into the viewer page."""
import json, pathlib
root = pathlib.Path(__file__).resolve().parent
games = json.loads((root.parent / "reports/showcase/games.json").read_text())
html = (root / "live_template.html").read_text().replace("/*GAMES*/[]", json.dumps(games, separators=(",", ":"))).replace("/*PIECES*/", (root / "pieces.css").read_text())
(root / "live.html").write_text(html)
print(len(games), "games,", len(html) // 1024, "KB")
