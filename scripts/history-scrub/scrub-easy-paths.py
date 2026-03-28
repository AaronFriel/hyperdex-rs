#!/usr/bin/env python3
from __future__ import annotations

import subprocess
from pathlib import Path


HOME_PREFIX = "/" + "home" + "/" + "friel"
TARGET = HOME_PREFIX
REPO_PREFIX = f"{HOME_PREFIX}/c/aaronfriel/hyperdex-rs/"
REPO_ROOT = REPO_PREFIX[:-1]
REPLACEMENTS = (
    (f"{HOME_PREFIX}/.cargo", "$CARGO_HOME"),
    (
        f"{HOME_PREFIX}/.codex/skills/execplan/references/PLANS.md",
        "the installed `execplan` skill fallback rules",
    ),
    (f"{HOME_PREFIX}/c/aaronfriel/HyperDex", "HyperDex"),
    (f"{HOME_PREFIX}/HyperDex", "HyperDex"),
    (f"{HOME_PREFIX}/c/aaronfriel/hyhac", "hyhac"),
    (f"{HOME_PREFIX}/c/aaronfriel/busybee-", "busybee-"),
    (f"{HOME_PREFIX}/c/aaronfriel/busybee", "busybee"),
    (
        f"{HOME_PREFIX}/.codex/skills/autoplan/references/PLANS.md",
        "the installed `autoplan` skill fallback rules",
    ),
    (
        f"{HOME_PREFIX}/.codex/skills/autoplan/references/AUTOPLANS.md",
        "the installed `autoplan` skill fallback rules",
    ),
    (f"{HOME_PREFIX}/.codex/", "$CODEX_HOME/"),
)
EXCLUDED_PREFIXES = ("docs/research/",)


def tracked_files() -> list[Path]:
    out = subprocess.check_output(["git", "ls-files", "-z"])
    return [Path(p) for p in out.decode().split("\0") if p]


def scrub_text(path_str: str, text: str) -> str:
    text = text.replace(REPO_PREFIX, "")
    text = text.replace(REPO_ROOT, "this repository")
    for old, new in REPLACEMENTS:
        text = text.replace(old, new)
    if path_str.startswith("scripts/history-scrub/"):
        text = text.replace(f'"{TARGET}"', '"/" + "home" + "/" + "friel"')
    return text


def main() -> int:
    changed = 0
    for path in tracked_files():
        path_str = path.as_posix()
        if path_str.startswith(EXCLUDED_PREFIXES):
            continue
        try:
            original = path.read_text()
        except UnicodeDecodeError:
            continue
        if TARGET not in original:
            continue
        rewritten = scrub_text(path_str, original)
        if rewritten == original:
            continue
        path.write_text(rewritten)
        changed += 1
    print(f"changed_files={changed}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
