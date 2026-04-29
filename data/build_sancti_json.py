#!/usr/bin/env python3
"""
Preprocess the Divinum Officium Latin Sancti/ corpus + the 1955+1960
kalendaria diffs into compact date-keyed JSON files we can ship as
single assets and look up at SSG build time.

Run once when the upstream data changes:

    python3 md2json2/data/build_sancti_json.py \
        --sancti      /path/to/divinum-officium-cgi-bin/data/horas/Latin/Sancti \
        --kalendaria  /path/to/divinum-officium-cgi-bin/data/Tabulae/Kalendaria \
        --sancti-out      md2json2/data/sancti.json \
        --kalendaria-out  md2json2/data/kalendaria_1962.json

The Sancti output JSON has the shape:

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

The Kalendaria 1962 output JSON resolves the 1955 + 1960 diffs and
gives the actual 1962 typical-edition calendar:

    {
      "MM-DD": null   // suppressed in 1962 (XXXXX in either diff),
      "MM-DD": {
        "main": { "name": str, "rank_num": float|null, "sancti_key": str|null },
        "commemorations": [{ "name": str, "rank_num": float, "sancti_key": str|null }, ...]
      }
    }

Dates *not present* in either diff inherit their original Divino
Afflatu (1954) entry — i.e. the default `[Rank]` from the Sancti file
is the right answer.
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


DATE_LINE_RE = re.compile(r"^(\d{2}-\d{2})=(.*)$")


def parse_kalendaria_file(text: str) -> dict:
    """Read a single Kalendaria diff file (1955.txt or 1960.txt) and
    return `{ "MM-DD": <entry-or-None> }`.
    `None` → `XXXXX` (suppressed).
    `entry` → `{ "main": {...}, "commemorations": [...] }`.
    Lines starting with `#` or wrapped in `*Month*` are ignored.
    """
    out: dict[str, dict | None] = {}
    for raw in text.splitlines():
        line = raw.rstrip()
        if not line or line.startswith("#") or line.lstrip().startswith("*"):
            continue
        m = DATE_LINE_RE.match(line.strip())
        if m is None:
            continue
        date = m.group(1)
        rhs = m.group(2)
        if rhs.strip() == "XXXXX":
            out[date] = None
            continue
        # Format: <keys>=<f1 name>=<f1 rank>=<f2 name>=<f2 rank>=...
        # `keys` itself may contain `~` to refer to multiple Sancti files
        # (one per feast/commemoratio). Trailing `=` is common.
        parts = rhs.split("=")
        if not parts:
            continue
        keys = [k for k in parts[0].split("~") if k]
        feasts: list[dict] = []
        i = 1
        while i < len(parts):
            name = parts[i].strip()
            rank_s = parts[i + 1].strip() if i + 1 < len(parts) else ""
            i += 2
            if not name or name == "XXXXX":
                continue
            try:
                rank = float(rank_s) if rank_s else None
            except ValueError:
                rank = None
            sancti_key = keys[len(feasts)] if len(feasts) < len(keys) else None
            feasts.append({
                "name": name,
                "rank_num": rank,
                "sancti_key": sancti_key,
            })
        if not feasts:
            out[date] = None
        else:
            out[date] = {"main": feasts[0], "commemorations": feasts[1:]}
    return out


def build_kalendaria_1962(kalendaria_dir: Path) -> dict:
    """Merge 1955 over 1960 — actually 1960 wins where it overrides
    1955. Dates absent from both keep their Divino-Afflatu defaults
    (i.e. fall through to the Sancti file at lookup time)."""
    p1955 = kalendaria_dir / "1955.txt"
    p1960 = kalendaria_dir / "1960.txt"
    if not p1955.is_file() or not p1960.is_file():
        print(f"error: missing 1955.txt or 1960.txt under {kalendaria_dir}",
              file=sys.stderr)
        sys.exit(1)
    base = parse_kalendaria_file(p1955.read_text(encoding="utf-8"))
    diff = parse_kalendaria_file(p1960.read_text(encoding="utf-8"))
    merged: dict = dict(base)
    for k, v in diff.items():
        merged[k] = v
    return merged


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sancti", required=True, type=Path,
                    help="Path to divinum-officium .../data/horas/Latin/Sancti")
    ap.add_argument("--kalendaria", required=True, type=Path,
                    help="Path to .../data/Tabulae/Kalendaria")
    ap.add_argument("--sancti-out", required=True, type=Path,
                    help="Where to write sancti.json")
    ap.add_argument("--kalendaria-out", required=True, type=Path,
                    help="Where to write kalendaria_1962.json")
    args = ap.parse_args()

    if not args.sancti.is_dir():
        print(f"error: {args.sancti} is not a directory", file=sys.stderr)
        sys.exit(1)
    if not args.kalendaria.is_dir():
        print(f"error: {args.kalendaria} is not a directory", file=sys.stderr)
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

    args.sancti_out.parent.mkdir(parents=True, exist_ok=True)
    args.sancti_out.write_text(
        json.dumps(by_date, ensure_ascii=False, indent=0,
                   separators=(",", ":")),
        encoding="utf-8",
    )
    print(f"sancti.json:        {len(by_date)} dates → {args.sancti_out} "
          f"({args.sancti_out.stat().st_size} bytes)")

    kal = build_kalendaria_1962(args.kalendaria)
    args.kalendaria_out.parent.mkdir(parents=True, exist_ok=True)
    args.kalendaria_out.write_text(
        json.dumps(kal, ensure_ascii=False, indent=0,
                   separators=(",", ":")),
        encoding="utf-8",
    )
    suppressed = sum(1 for v in kal.values() if v is None)
    overrides = len(kal) - suppressed
    print(f"kalendaria_1962:    {overrides} overrides + {suppressed} suppressions "
          f"→ {args.kalendaria_out} ({args.kalendaria_out.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
