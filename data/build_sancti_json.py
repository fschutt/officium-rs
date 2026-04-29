#!/usr/bin/env python3
"""
Preprocess the Divinum Officium Latin Sancti/ corpus into a compact
date-keyed JSON we can ship as a single asset and look up at SSG build
time.

Run once when the upstream Sancti data changes:

    python3 md2json2/data/build_sancti_json.py \
        --sancti /path/to/divinum-officium-cgi-bin/data/horas/Latin/Sancti \
        --out    md2json2/data/sancti.json

The output JSON has the shape:

    {
      "MM-DD": [
        {
          "rubric": "default" | "1960" | "1960_aut_innovata" | "196" | "<other>",
          "name":     str,
          "rank_class": str,           # Duplex / Semiduplex / Simplex / etc.
          "rank_num":   float | null,  # numeric precedence rank
          "commune":    str            # Common reference (e.g., "ex C1")
        },
        ...
      ]
    }

Only base files (`MM-DD.txt`) are included — suffixed variants
(`MM-DD<suffix>.txt`) are referenced by the precedence/concurrence
machinery, which is out of scope for the first sanctoral lookup pass.
"""

from __future__ import annotations
import argparse
import json
import re
import sys
from pathlib import Path

BASE_FILE_RE = re.compile(r"^(\d{2})-(\d{2})\.txt$")
RANK_HEADER_RE = re.compile(
    r"^\[Rank\](?:\s*\(\s*(?:rubrica\s+)?([^)]+?)\s*\))?\s*$",
    re.IGNORECASE,
)


def classify_rubric(label: str | None) -> str:
    """Map the parenthetical after [Rank] to a stable id our Rust code
    can match on."""
    if label is None:
        return "default"
    s = label.strip().lower()
    # Accept variations: "1960", "rubrica 1960", "1960 aut rubrica innovata".
    if "1960 aut" in s or "innovata" in s:
        return "1960_aut_innovata"
    if "1960" in s:
        return "1960"
    if s in {"196", "rubrica 196"} or s.startswith("196 "):
        return "196"
    return s.replace(" ", "_")


def parse_rank_block(body: list[str]) -> dict | None:
    """Pull the first non-comment-looking entry line out of a [Rank] body.

    Format (from the Sancti corpus):
        Name;;Rank-class;;Rank-num;;CommuneRef
    Lines starting with `(` are alt-rubric markers and skipped here —
    they introduce the next [Rank] block above us instead.
    """
    for raw in body:
        line = raw.strip()
        if not line:
            continue
        # Old-style alt-rubric markers like "(sed rubrica 1617)" appear
        # as their own lines and signal the *next* [Rank] block; ignore.
        if line.startswith("(") and line.endswith(")"):
            continue
        parts = [p.strip() for p in line.split(";;")]
        if not parts or not parts[0]:
            continue
        name = parts[0]
        rank_class = parts[1] if len(parts) > 1 else ""
        rank_num: float | None = None
        if len(parts) > 2 and parts[2]:
            try:
                rank_num = float(parts[2])
            except ValueError:
                rank_num = None
        commune = parts[3] if len(parts) > 3 else ""
        return {
            "name": name,
            "rank_class": rank_class,
            "rank_num": rank_num,
            "commune": commune,
        }
    return None


def parse_sancti_file(text: str) -> list[dict]:
    """Walk the file, splitting on `[Section]` headers, and emit one
    entry per [Rank] variant block found."""
    entries: list[dict] = []
    current_label: str | None = None
    current_body: list[str] | None = None
    in_rank = False

    def flush():
        nonlocal current_label, current_body, in_rank
        if in_rank and current_body is not None:
            parsed = parse_rank_block(current_body)
            if parsed is not None:
                parsed["rubric"] = classify_rubric(current_label)
                entries.append(parsed)
        current_label = None
        current_body = None
        in_rank = False

    for line in text.splitlines():
        m = RANK_HEADER_RE.match(line)
        if m is not None:
            flush()
            current_label = m.group(1)
            current_body = []
            in_rank = True
            continue
        if line.startswith("[") and "]" in line and in_rank:
            # next section, stop accumulating Rank
            flush()
            continue
        if in_rank and current_body is not None:
            current_body.append(line)
    flush()
    return entries


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sancti", required=True, type=Path,
                    help="Path to divinum-officium .../data/horas/Latin/Sancti")
    ap.add_argument("--out", required=True, type=Path,
                    help="Path to write the compact JSON")
    args = ap.parse_args()

    if not args.sancti.is_dir():
        print(f"error: {args.sancti} is not a directory", file=sys.stderr)
        sys.exit(1)

    by_date: dict[str, list[dict]] = {}
    for path in sorted(args.sancti.iterdir()):
        m = BASE_FILE_RE.match(path.name)
        if m is None:
            continue
        key = f"{m.group(1)}-{m.group(2)}"
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            text = path.read_text(encoding="latin-1")
        entries = parse_sancti_file(text)
        if entries:
            by_date[key] = entries

    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(by_date, ensure_ascii=False, indent=0,
                                   separators=(",", ":")), encoding="utf-8")
    print(f"wrote {len(by_date)} dates → {args.out} "
          f"({args.out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
