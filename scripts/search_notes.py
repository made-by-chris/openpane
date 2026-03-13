#!/usr/bin/env python3

from __future__ import annotations

import argparse
from pathlib import Path


DEFAULT_ROOTS = [
    Path(r"C:\Users\Christopher\projects\main"),
    Path(r"C:\Users\Christopher\projects\ANGEL"),
    Path(r"C:\Users\Christopher\projects\AI\old\picnic\picnic"),
]

DEFAULT_KEYWORDS = [
    "npm",
    "2fa",
    "two factor",
    "authenticator",
    "otp",
    "totp",
    "recovery",
    "recovery code",
    "backup code",
    "backup codes",
    "npmjs",
    "publish",
    "token",
    "login",
]

TEXT_EXTENSIONS = {
    ".md",
    ".txt",
    ".json",
    ".canvas",
    ".yaml",
    ".yml",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Search likely note files for 2FA/npm-related keywords."
    )
    parser.add_argument(
        "roots",
        nargs="*",
        type=Path,
        help="Directories to search. Defaults to detected Obsidian vaults.",
    )
    parser.add_argument(
        "--keyword",
        dest="keywords",
        action="append",
        default=[],
        help="Add an extra keyword to search for. Can be repeated.",
    )
    parser.add_argument(
        "--extensions",
        nargs="*",
        default=sorted(TEXT_EXTENSIONS),
        help="File extensions to include.",
    )
    parser.add_argument(
        "--max-results",
        type=int,
        default=200,
        help="Maximum number of matches to print.",
    )
    return parser.parse_args()


def iter_files(root: Path, extensions: set[str]):
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix.lower() not in extensions:
            continue
        yield path


def search_file(path: Path, keywords: list[str]):
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return []

    lowered = text.lower()
    hits = []

    for keyword in keywords:
        start = 0
        needle = keyword.lower()
        while True:
            index = lowered.find(needle, start)
            if index == -1:
                break

            line_start = lowered.rfind("\n", 0, index) + 1
            line_end = lowered.find("\n", index)
            if line_end == -1:
                line_end = len(text)

            line_number = lowered.count("\n", 0, index) + 1
            excerpt = text[line_start:line_end].strip()
            hits.append((keyword, line_number, excerpt))
            start = index + len(needle)

    return hits


def main() -> int:
    args = parse_args()
    roots = args.roots or DEFAULT_ROOTS
    keywords = DEFAULT_KEYWORDS + args.keywords
    extensions = {ext if ext.startswith(".") else f".{ext}" for ext in args.extensions}

    printed = 0

    for root in roots:
        if not root.exists():
            continue

        print(f"\n== Searching {root} ==")

        for path in iter_files(root, extensions):
            hits = search_file(path, keywords)
            if not hits:
                continue

            print(f"\n{path}")
            for keyword, line_number, excerpt in hits:
                safe_excerpt = excerpt[:220].encode("cp1252", errors="replace").decode("cp1252")
                print(f"  [{keyword}] line {line_number}: {safe_excerpt}")
                printed += 1
                if printed >= args.max_results:
                    print("\nReached max results.")
                    return 0

    if printed == 0:
        print("No matches found.")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
