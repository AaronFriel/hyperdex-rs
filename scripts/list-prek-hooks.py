#!/usr/bin/env python3
import json
import re
from pathlib import Path


CONFIG_PATH = Path(".pre-commit-config.yaml")
HOOK_ID_RE = re.compile(r"^\s*-\s+id:\s+(.+?)\s*$")
STAGES_RE = re.compile(r"^\s*stages:\s*\[(.*?)\]\s*$")


def parse_hooks() -> list[dict[str, object]]:
    hooks: list[dict[str, object]] = []
    current: dict[str, object] | None = None

    for raw_line in CONFIG_PATH.read_text().splitlines():
        hook_match = HOOK_ID_RE.match(raw_line)
        if hook_match:
            if current is not None:
                hooks.append(current)
            current = {"id": hook_match.group(1), "stages": None}
            continue

        if current is None:
            continue

        stages_match = STAGES_RE.match(raw_line)
        if stages_match:
            stages = [
                stage.strip().strip("'\"")
                for stage in stages_match.group(1).split(",")
                if stage.strip()
            ]
            current["stages"] = stages

    if current is not None:
        hooks.append(current)

    return hooks


def hook_record(hook_id: str) -> dict[str, object]:
    return {
        "id": hook_id,
        "needs_rust": hook_id in {"rustfmt-check", "nextest-fast", "clippy"},
        "needs_rustfmt": hook_id == "rustfmt-check",
        "needs_clippy": hook_id == "clippy",
        "needs_gitleaks": hook_id == "gitleaks",
        "needs_system_packages": hook_id in {"nextest-fast", "clippy"},
        "needs_nextest": hook_id == "nextest-fast",
    }


def main() -> None:
    hooks = []
    for hook in parse_hooks():
        stages = hook["stages"]
        if stages is not None and "pre-commit" not in stages:
            continue
        hooks.append(hook_record(hook["id"]))

    print(json.dumps({"include": hooks}))


if __name__ == "__main__":
    main()
