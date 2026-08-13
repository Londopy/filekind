#!/usr/bin/env python3
"""Extract one release's section from CHANGELOG.md, verbatim.

`patchnotes ... latest` renders a plain-text summary, which is exactly right in
a terminal and exactly wrong in a GitHub release body: its indented bullets
turn into a code block. This pulls the original markdown out untouched, so the
release notes and the changelog are the same bytes.

patchnotes still owns validation and version sync in the workflow — this only
does the slicing.

    python3 scripts/release-notes.py v0.5.0 [CHANGELOG.md]

Exits 1 if the version has no section, which is the correct way to fail a
release that would otherwise ship with empty notes.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

HEADING = re.compile(r"^## +\[?([^\]\s]+)\]?", re.M)


def extract(text: str, version: str) -> str | None:
    wanted = version.lstrip("vV")
    matches = list(HEADING.finditer(text))
    for i, m in enumerate(matches):
        if m.group(1).lstrip("vV") != wanted:
            continue
        start = m.end()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        body = text[start:end]
        # Drop the rest of the heading line (the "- 2026-08-13" part) and any
        # link-reference footnotes that trail the final section.
        body = body.split("\n", 1)[1] if "\n" in body else ""
        body = re.sub(r"^\[[^\]]+\]:\s*\S+$", "", body, flags=re.M)
        return body.strip("\n")
    return None


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2

    version = sys.argv[1]
    path = Path(sys.argv[2] if len(sys.argv) > 2 else "CHANGELOG.md")

    body = extract(path.read_text(encoding="utf-8"), version)
    if body is None:
        print(f"release-notes: no section for {version} in {path}", file=sys.stderr)
        return 1

    print(body)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
