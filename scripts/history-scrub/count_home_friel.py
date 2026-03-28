#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


HOME_PREFIX = "/" + "home" + "/" + "friel"
TARGET = HOME_PREFIX
REPO_PREFIX = f"{HOME_PREFIX}/c/aaronfriel/hyperdex-rs/"
REPO_ROOT = REPO_PREFIX[:-1]
EXTERNAL_LOCAL_PREFIXES = (
    f"{HOME_PREFIX}/c/aaronfriel/HyperDex",
    f"{HOME_PREFIX}/c/aaronfriel/hyhac",
    f"{HOME_PREFIX}/c/aaronfriel/busybee",
    f"{HOME_PREFIX}/c/aaronfriel/busybee-",
    f"{HOME_PREFIX}/HyperDex",
    f"{HOME_PREFIX}/.cargo/",
)
EXCLUDED_PREFIXES = ("docs/research/",)
NEUTRAL_LOCAL_PREFIXES = (f"{HOME_PREFIX}/.codex/", REPO_PREFIX, REPO_ROOT)


@dataclass
class CountResult:
    total: int = 0
    repo_local: int = 0
    external_local: int = 0
    excluded: int = 0


def repo_root() -> Path:
    return Path(
        subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], text=True
        ).strip()
    )


def classify(path: str, line: str) -> str:
    if path.startswith(EXCLUDED_PREFIXES):
        return "excluded"
    if any(prefix in line for prefix in EXTERNAL_LOCAL_PREFIXES):
        return "external_local"
    if any(prefix in line for prefix in NEUTRAL_LOCAL_PREFIXES):
        return "repo_local"
    if TARGET in line:
        return "external_local"
    return "repo_local"


def add_match(result: CountResult, bucket: str) -> None:
    result.total += 1
    if bucket == "repo_local":
        result.repo_local += 1
    elif bucket == "external_local":
        result.external_local += 1
    elif bucket == "excluded":
        result.excluded += 1


def count_tree(root: Path) -> CountResult:
    proc = subprocess.run(
        ["git", "grep", "-n", TARGET, "--", "."],
        cwd=root,
        text=True,
        capture_output=True,
        check=False,
    )
    result = CountResult()
    if proc.returncode not in (0, 1):
        raise RuntimeError(proc.stderr.strip() or "git grep failed")
    for raw in proc.stdout.splitlines():
        path, _, line = raw.partition(":")
        bucket = classify(path, raw)
        add_match(result, bucket)
    return result


def count_history(root: Path, all_refs: bool) -> CountResult:
    rev_args = ["--all"] if all_refs else ["HEAD"]
    objects = subprocess.check_output(
        ["git", "rev-list", "--objects", *rev_args], cwd=root, text=True
    ).splitlines()
    entries: list[tuple[str, str]] = []
    for line in objects:
        oid, *rest = line.split(" ", 1)
        if not rest:
            continue
        path = rest[0]
        if path.startswith(EXCLUDED_PREFIXES):
            continue
        entries.append((oid, path))

    if not entries:
        return CountResult()

    batch_check = subprocess.check_output(
        ["git", "cat-file", "--batch-check"],
        cwd=root,
        input="".join(f"{oid}\n" for oid, _ in entries),
        text=True,
    ).splitlines()

    blob_kinds = {
        parts[0]: parts[1]
        for raw in batch_check
        if len((parts := raw.split(" ", 2))) >= 2
    }
    unique_blob_ids = sorted({oid for oid, _ in entries if blob_kinds.get(oid) == "blob"})

    result = CountResult()
    lines_by_oid: dict[str, list[str]] = {}
    proc = subprocess.Popen(
        ["git", "cat-file", "--batch"],
        cwd=root,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert proc.stdin is not None
    assert proc.stdout is not None
    for oid in unique_blob_ids:
        proc.stdin.write(f"{oid}\n".encode())
    proc.stdin.flush()
    proc.stdin.close()

    for oid in unique_blob_ids:
        header = proc.stdout.readline().decode().strip()
        _, kind, size_text = header.split(" ", 2)
        if kind != "blob":
            raise RuntimeError(f"expected blob for {oid}, got {kind}")
        size = int(size_text)
        data = proc.stdout.read(size)
        proc.stdout.read(1)
        if TARGET.encode() not in data:
            lines_by_oid[oid] = []
            continue
        lines_by_oid[oid] = [
            raw_line.decode("utf-8", errors="replace")
            for raw_line in data.splitlines()
            if TARGET.encode() in raw_line
        ]

    stderr = proc.stderr.read().decode()
    if proc.wait() != 0:
        raise RuntimeError(stderr.strip() or "git cat-file --batch failed")

    for oid, path in entries:
        for line in lines_by_oid.get(oid, []):
            bucket = classify(path, line)
            add_match(result, bucket)

    return result


def print_result(mode: str, result: CountResult) -> None:
    print(f"mode={mode}")
    print(f"total_refs={result.total}")
    print(f"repo_local_refs={result.repo_local}")
    print(f"external_local_refs={result.external_local}")
    print(f"excluded_refs={result.excluded}")


def main() -> int:
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--tree", action="store_true")
    group.add_argument("--history", action="store_true")
    parser.add_argument("--all-refs", action="store_true")
    args = parser.parse_args()

    root = repo_root()
    os.chdir(root)

    result = count_tree(root) if args.tree else count_history(root, args.all_refs)
    print_result("tree" if args.tree else "history", result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
