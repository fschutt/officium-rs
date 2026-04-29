#!/usr/bin/env python3
"""
Preprocess the Divinum Officium Latin Mass corpus into a single
keyed JSON we ship as a build asset for /wip/missal.

Run once when the upstream Mass data changes:

    python3 md2json2/data/build_missa_json.py \
        --missa-latin /tmp/do-upstream/web/www/missa/Latin \
        --out         md2json2/data/missa_latin.json

Output shape:

    {
      "Sancti/04-29": {
        "officium": "S. Petri Martyris",                 // when [Officium]
        "rank":     "Duplex",
        "rank_num": 3.0,
        "commune":  "vide C2a-1",
        "sections": {                                    // raw section bodies
          "Oratio": "Præsta, quǽsumus ...",
          "Lectio": "@Commune/C2a-1",
          "Secreta": "...",
          "Postcommunio": "...",
          ...
        }
      },
      "Tempora/Pasc3-0": { ... },
      "Commune/C2a-1": { ... },
      ...
    }

The keys mirror the upstream filenames so callers can resolve
`@Commune/Cxx-y` references with a simple `data["Commune/Cxx-y"]`
lookup.

Section bodies are kept as raw strings (with `\n` newlines preserved).
The renderer in md2json2/src/missal.rs takes care of CommonMark-ish
escaping; we do not pre-process bodies here so the upstream conventions
(`!Ps 65:1-2.` reading citations, `$Per Dominum` macros, `&Gloria`
inserts) survive intact for downstream processing.
"""

from __future__ import annotations
import argparse
import json
import re
import sys
from pathlib import Path

SECTION_RE = re.compile(r"^\[([^\]]+)\]\s*$")
BASE_FILE_RE = re.compile(r"\.txt$")


def parse_mass_file(text: str) -> dict:
    """Split a Mass file into a dict-of-sections plus a parsed [Rank]
    summary. The [Rank] body is stored separately because it isn't a
    proper section — it carries metadata (rank class / numeric rank /
    Commune ref) rather than printable Mass text."""
    sections: dict[str, list[str]] = {}
    current = None
    for raw in text.splitlines():
        m = SECTION_RE.match(raw.rstrip())
        if m is not None:
            current = m.group(1).strip()
            sections.setdefault(current, [])
            continue
        if current is not None:
            sections[current].append(raw)

    out: dict = {}
    if "Officium" in sections:
        out["officium"] = " ".join(s.strip() for s in sections.pop("Officium")).strip() or None
    if "Rank" in sections:
        rank_body = [ln.strip() for ln in sections.pop("Rank") if ln.strip()
                     and not (ln.strip().startswith("(") and ln.strip().endswith(")"))]
        if rank_body:
            parts = [p.strip() for p in rank_body[0].split(";;")]
            if not out.get("officium") and parts and parts[0]:
                out["officium"] = parts[0]
            out["rank"] = parts[1] if len(parts) > 1 else None
            try:
                out["rank_num"] = float(parts[2]) if len(parts) > 2 and parts[2] else None
            except ValueError:
                out["rank_num"] = None
            out["commune"] = parts[3] if len(parts) > 3 else None
    # Keep all remaining sections as joined strings so the renderer can
    # treat `\n` as a soft separator (matching the upstream convention).
    out["sections"] = {
        name: "\n".join(body).strip() for name, body in sections.items()
        if "\n".join(body).strip()
    }
    return out


def gather(missa_root: Path) -> dict:
    """Walk Sancti/, Tempora/, Commune/, Ordo/ under the Latin Mass
    root and pack each file into the keyed JSON."""
    out: dict = {}
    for subdir in ("Sancti", "Tempora", "Commune", "Ordo"):
        dir_path = missa_root / subdir
        if not dir_path.is_dir():
            continue
        for path in sorted(dir_path.iterdir()):
            if not BASE_FILE_RE.search(path.name):
                continue
            stem = path.stem  # MM-DD or Pasc3-0 or C2a-1 etc.
            key = f"{subdir}/{stem}"
            try:
                text = path.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                text = path.read_text(encoding="latin-1")
            parsed = parse_mass_file(text)
            # skip files that produced literally nothing — keeps the
            # JSON down to "actual content" only.
            if parsed.get("sections") or parsed.get("officium"):
                out[key] = parsed
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--missa-latin", required=True, type=Path,
                    help="Path to <upstream>/web/www/missa/Latin")
    ap.add_argument("--horas-commune", type=Path, default=None,
                    help="Path to <upstream>/web/www/horas/Latin/Commune "
                         "(upstream stores Common files here, shared between "
                         "Office and Mass; @Commune/Cxx refs resolve here)")
    ap.add_argument("--out", required=True, type=Path,
                    help="Where to write missa_latin.json")
    args = ap.parse_args()

    if not args.missa_latin.is_dir():
        print(f"error: {args.missa_latin} is not a directory", file=sys.stderr)
        sys.exit(1)

    data = gather(args.missa_latin)
    if args.horas_commune is not None:
        if not args.horas_commune.is_dir():
            print(f"error: {args.horas_commune} is not a directory", file=sys.stderr)
            sys.exit(1)
        # Stuff the shared Office Commune into the same map under
        # the same `Commune/Cxx` keys our Mass `@Commune/Cxx` references
        # use. The upstream code resolves both ways to the same file.
        for path in sorted(args.horas_commune.iterdir()):
            if not BASE_FILE_RE.search(path.name):
                continue
            key = f"Commune/{path.stem}"
            try:
                text = path.read_text(encoding="utf-8")
            except UnicodeDecodeError:
                text = path.read_text(encoding="latin-1")
            parsed = parse_mass_file(text)
            if parsed.get("sections") or parsed.get("officium"):
                data[key] = parsed
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(
        json.dumps(data, ensure_ascii=False, indent=0,
                   separators=(",", ":")),
        encoding="utf-8",
    )
    by_dir = {}
    for k in data:
        d = k.split("/", 1)[0]
        by_dir[d] = by_dir.get(d, 0) + 1
    print(f"wrote {len(data)} keys → {args.out} "
          f"({args.out.stat().st_size} bytes)")
    for d, n in sorted(by_dir.items()):
        print(f"   {d:<8} {n} files")


if __name__ == "__main__":
    main()
