#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

python3 - "$REPO_ROOT" <<'PY'
from pathlib import Path
import re
import sys

repo_root = Path(sys.argv[1])
agents_path = repo_root / "AGENTS.md"
agents_text = agents_path.read_text()
threshold_match = re.search(r"exceeds\s+(\d+)\s+lines", agents_text)
if not threshold_match:
    raise SystemExit(f"could not determine line threshold from {agents_path}")
threshold = int(threshold_match.group(1))

module_pattern = re.compile(
    r"(?m)^\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{"
)

def is_generated_rust(path: Path) -> bool:
    try:
        with path.open() as handle:
            for _ in range(3):
                line = handle.readline()
                if not line:
                    break
                if "@generated" in line:
                    return True
    except OSError:
        return False
    return False

violations = []
for path in sorted((repo_root / "crates").rglob("*.rs")):
    if is_generated_rust(path):
        continue
    text = path.read_text()
    line_count = text.count("\n")
    if text and not text.endswith("\n"):
        line_count += 1
    if line_count <= threshold:
        continue

    for match in module_pattern.finditer(text):
        line = text.count("\n", 0, match.start()) + 1
        violations.append((path.relative_to(repo_root), line_count, line, match.group(0).strip()))

if violations:
    print(
        f"inline submodule layout violations in files over {threshold} lines:",
        file=sys.stderr,
    )
    for rel_path, line_count, line, snippet in violations:
        print(
            f"  {rel_path}:{line} ({line_count} lines): {snippet}",
            file=sys.stderr,
        )
    raise SystemExit(1)

print(f"module layout ok for threshold {threshold}")
PY
