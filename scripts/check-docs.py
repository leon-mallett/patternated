#!/usr/bin/env python3
"""Every relative link and anchor in the documentation set must resolve.

**Exits non-zero when one does not**, which is the whole point of it existing as
a file rather than as a line of shell. An earlier inline version printed its
problems and exited zero, so a chain that ran it before committing duly
reported the breakage and then committed anyway.

The anchor rules match GitHub's: lowercase, drop punctuation, then turn each
space into a hyphen, and note *each*, so the double space left either side of a
removed em dash becomes a double hyphen. Getting that wrong reports correct
links as broken, which is how the first attempt at this wasted a round.
"""

import pathlib
import re
import sys


def slug(text: str) -> str:
    cleaned = re.sub(r"[^\w\s-]", "", text.strip().lower())
    return cleaned.replace(" ", "-")


def anchors(path: pathlib.Path) -> set:
    headings = (re.match(r"^#{1,6}\s+(.*)", line) for line in path.read_text().splitlines())
    return {slug(m.group(1)) for m in headings if m}


def main() -> int:
    root = pathlib.Path(__file__).resolve().parent.parent
    pages = [
        *sorted(root.glob("docs/*.md")),
        root / "README.md",
        *sorted(root.glob("examples/*/*.md")),
    ]

    problems = []
    for page in pages:
        for _, link in re.findall(r"\[([^\]]+)\]\((\.[^)]+)\)", page.read_text()):
            path, _, fragment = link.partition("#")
            target = (page.parent / path).resolve()
            where = page.relative_to(root)
            if not target.exists():
                problems.append(f"{where} -> {link}  (no such file)")
            elif fragment and fragment not in anchors(target):
                problems.append(f"{where} -> {link}  (no such heading)")

    if problems:
        print("broken documentation links:")
        for problem in problems:
            print(f"  {problem}")
        return 1

    print(f"all links and anchors resolve across {len(pages)} pages")
    return 0


if __name__ == "__main__":
    sys.exit(main())
