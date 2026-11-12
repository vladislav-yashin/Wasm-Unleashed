#!/usr/bin/env python3
"""Generate the self-contained packaging/web/deploy/setup-server.sh.

The generated script embeds the production compose file, the remote deploy
helper and the default deploy environment. Only the generated script has to be
copied to a server.

Run this after changing any embedded source, or use --check in CI.
"""
from __future__ import annotations

import argparse
import difflib
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
WEB_DIR = HERE.parent
TEMPLATE = HERE / "setup-server.sh.in"
OUTPUT = HERE / "setup-server.sh"

EMBEDS = (
    (
        "@@EMBED_COMPOSE@@",
        WEB_DIR / "compose.deploy.yaml",
        "OPEN_UNLEASHED_COMPOSE_EOF",
        'cat > "$COMPOSE_FILE"',
        'chmod 0644 "$COMPOSE_FILE"',
    ),
    (
        "@@EMBED_REMOTE@@",
        HERE / "remote-deploy.sh",
        "OPEN_UNLEASHED_REMOTE_EOF",
        'cat > "$DEPLOY_SCRIPT"',
        'chmod 0755 "$DEPLOY_SCRIPT"',
    ),
    (
        "@@EMBED_ENV@@",
        HERE / "deploy.env.example",
        "OPEN_UNLEASHED_ENV_EOF",
        'cat > "$ENV_FILE"',
        'chmod 0644 "$ENV_FILE"',
    ),
)


def heredoc_block(path: Path, delimiter: str, command: str, chmod: str) -> str:
    content = path.read_text(encoding="utf-8")
    if not content.endswith("\n"):
        content += "\n"
    if any(line == delimiter for line in content.splitlines()):
        raise SystemExit(f"{path}: content contains heredoc delimiter {delimiter}")
    return f"{command} <<'{delimiter}'\n{content}{delimiter}\n{chmod}"


def render() -> str:
    text = TEMPLATE.read_text(encoding="utf-8")
    for marker, path, delimiter, command, chmod in EMBEDS:
        needle = f"# {marker}"
        if text.count(needle) != 1:
            raise SystemExit(f"{TEMPLATE}: expected exactly one {needle}")
        text = text.replace(needle, heredoc_block(path, delimiter, command, chmod))
    return text


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify that setup-server.sh matches the embedded sources",
    )
    args = parser.parse_args()

    expected = render()
    if args.check:
        try:
            actual = OUTPUT.read_text(encoding="utf-8")
        except FileNotFoundError:
            print(f"{OUTPUT}: missing; run {Path(__file__).name}", file=sys.stderr)
            return 1
        if actual != expected:
            print(f"{OUTPUT} is out of date.", file=sys.stderr)
            sys.stderr.writelines(
                difflib.unified_diff(
                    actual.splitlines(keepends=True),
                    expected.splitlines(keepends=True),
                    fromfile=str(OUTPUT),
                    tofile=f"{OUTPUT} (generated)",
                )
            )
            return 1
        print(f"{OUTPUT.name} is up to date.")
        return 0

    OUTPUT.write_text(expected, encoding="utf-8")
    OUTPUT.chmod(0o755)
    print(f"Wrote {OUTPUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
