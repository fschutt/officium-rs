#!/usr/bin/env python3
"""
Parse upstream's Tabulae/Kalendaria/<rubric>.txt files into a
temporal index mapping (rubric_layer, MM-DD) → KalendarEntry.

The kalendar files are SUPERSEDING layers — each lists the
differences from the previous canonical state. By parsing them in
chronological order and tracking when each saint/feast first
appears, we derive a canonization-date table for the year-aware
corpus filter.

Usage:
  python3 data/build_canonization.py \\
      --tabulae /path/to/vendor/divinum-officium/web/www/Tabulae/Kalendaria \\
      --out data/canonization_dates.json

File format:
  *Month name in stars*
  MM-DD=stem[~comm-stem]=Officium[=rank][=Comm-Officium=comm-rank]
  ...

  Where rank ∈ {1..7}:
    1 = Simplex
    2 = Semiduplex
    3 = Duplex
    4 = Duplex majus
    5 = Duplex II classis
    6 = Duplex I classis
    7 = Duplex I privilegiata

The Tabulae layers we care about (chronological order):
  1570 → 1888 → 1906 → 1939 → 1954 → 1955 → 1960
  (1955 = Pius XII Reduced; 1960 = John XXIII Rubrics)

Monastic / Cistercian / Dominican variants are separate columns:
  M1617, M1930, M1963, M1963B, C1951, OP1962, NC, CAV
"""

import argparse
import json
import re
import sys
from pathlib import Path
from collections import defaultdict


RUBRIC_ORDER = [
    ("1570", "Pius V baseline"),
    ("1888", "Pius IX / Leo XIII era"),
    ("1906", "Pius X early reforms"),
    ("1939", "Pius XI (Christ the King 1925, etc.)"),
    ("1954", "Pius XII pre-Reduced"),
    ("1955", "Pius XII Reduced (Cum nostra hac aetate)"),
    ("1960", "John XXIII Rubrics"),
]

RANK_LABEL = {
    "1": "Simplex",
    "1.5": "Vigilia",
    "2": "Semiduplex",
    "3": "Duplex",
    "4": "Duplex majus",
    "5": "Duplex II classis",
    "6": "Duplex I classis",
    "7": "Duplex I privilegiata",
}


def parse_kalendar_file(text: str) -> dict:
    """Parse a single Tabulae/Kalendaria/X.txt into a dict
    keyed by MM-DD with value = list-of-cells OR the literal
    sentinel `"SUPPRESSED"` when the day was zero'd out.

    Each cell is `{stem, officium, rank, kind}` where kind ∈
    {"main", "commemoratio"}.

    Note: the 1570 baseline lists every kalendar day; later
    layers list only the diff from the prior layer. A day that
    later layers do NOT mention is unchanged.

    Suppression marker: `MM-DD=XXXXX` (or trailing `=`) zeroes
    the day. We surface that as `"SUPPRESSED"`.
    """
    out: dict[str, object] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or line.startswith("*"):
            continue
        m = re.match(r"^(\d\d-\d\d)=(.*)$", line)
        if not m:
            continue
        mmdd = m.group(1)
        rest = m.group(2)
        # Suppression: `MM-DD=XXXXX[=]` or `MM-DD=X[=]`.
        first_field = rest.split("=", 1)[0].strip()
        if first_field.upper() in ("X", "XX", "XXX", "XXXX", "XXXXX"):
            out[mmdd] = "SUPPRESSED"
            continue
        fields = rest.split("=")
        # Format options:
        #   stem[~comm-stems]=Officium=rank[=Comm-Officium=comm-rank]
        if not fields:
            continue
        stems_raw = fields[0]
        officium = fields[1] if len(fields) > 1 else ""
        rank = fields[2] if len(fields) > 2 else ""
        comm_officium = fields[3] if len(fields) > 3 else ""
        comm_rank = fields[4] if len(fields) > 4 else ""
        stems = stems_raw.split("~")
        main_stem = stems[0]
        comm_stems = stems[1:]

        cells = []
        cells.append({
            "stem": main_stem,
            "officium": officium.strip(),
            "rank": rank.strip(),
            "rank_label": RANK_LABEL.get(rank.strip(), ""),
            "kind": "main",
        })
        for cs in comm_stems:
            cells.append({
                "stem": cs,
                "officium": comm_officium.strip(),
                "rank": comm_rank.strip(),
                "rank_label": RANK_LABEL.get(comm_rank.strip(), ""),
                "kind": "commemoratio",
            })
        out[mmdd] = cells
    return out


def resolve_layer(prior: dict, diff: dict) -> dict:
    """Apply a diff layer on top of a prior resolved kalendar.

    `prior`: dict[mmdd → list-of-cells].
    `diff`: dict[mmdd → list-of-cells | "SUPPRESSED"].

    Returns: new resolved kalendar (dict[mmdd → list-of-cells]).
    """
    resolved = dict(prior)
    for mmdd, val in diff.items():
        if val == "SUPPRESSED":
            resolved.pop(mmdd, None)
        else:
            resolved[mmdd] = val
    return resolved


def derive_canonization(tabulae_dir: Path) -> tuple[dict, dict]:
    """Walk the rubric layers in chronological order, build the
    resolved kalendar at each layer, and derive per-stem
    canonization metadata.

    Returns (canonization_table, resolved_kalendaria) where:
      - canonization_table[Sancti/<stem>]: per-stem facts
      - resolved_kalendaria[<rubric>][<mmdd>]: list-of-cells
        (the cumulative resolved kalendar at that rubric).
    """
    layers: dict[str, dict] = {}
    for rubric, label in RUBRIC_ORDER:
        path = tabulae_dir / f"{rubric}.txt"
        if not path.exists():
            print(f"warning: {path} missing", file=sys.stderr)
            continue
        text = path.read_text(encoding="utf-8")
        layers[rubric] = parse_kalendar_file(text)
        print(
            f"  {rubric}.txt: {len(layers[rubric])} dates, {label}",
            file=sys.stderr,
        )

    # Resolved kalendar at each rubric (cumulative).
    resolved_at: dict[str, dict] = {}
    cumulative: dict = {}
    for rubric, _label in RUBRIC_ORDER:
        cumulative = resolve_layer(cumulative, layers.get(rubric, {}))
        resolved_at[rubric] = cumulative

    # Per-stem history: in which rubrics is this stem live, and how?
    stem_appearances: dict[str, list] = defaultdict(list)
    for rubric, _label in RUBRIC_ORDER:
        kalendar = resolved_at[rubric]
        for mmdd, cells in kalendar.items():
            for cell in cells:
                stem = cell["stem"]
                if not stem:
                    continue
                stem_appearances[stem].append({
                    "rubric": rubric,
                    "mmdd": mmdd,
                    "officium": cell["officium"],
                    "rank": cell["rank"],
                    "kind": cell["kind"],
                })

    # Detect explicit suppressions: a date going from a stem to
    # `SUPPRESSED` in a layer's diff means the stem was removed.
    explicit_suppressions: dict[str, str] = {}  # stem -> rubric removed in
    for rubric, _label in RUBRIC_ORDER:
        diff = layers.get(rubric, {})
        for mmdd, val in diff.items():
            if val != "SUPPRESSED":
                continue
            # Find which stem(s) were on this date in the previous
            # rubric's resolved kalendar.
            idx = [r for r, _ in RUBRIC_ORDER].index(rubric)
            if idx == 0:
                continue
            prev_rubric = RUBRIC_ORDER[idx - 1][0]
            prior_cells = resolved_at[prev_rubric].get(mmdd, [])
            for cell in prior_cells:
                stem = cell["stem"]
                if stem and stem not in explicit_suppressions:
                    explicit_suppressions[stem] = rubric

    canonization: dict[str, dict] = {}
    for stem, hist in sorted(stem_appearances.items()):
        if not hist:
            continue
        first = hist[0]
        # Last layer where this stem is live in the resolved kalendar.
        last = hist[-1]
        # Track rank changes across layers.
        rank_history = []
        seen_rank = None
        last_rubric_seen = None
        for h in hist:
            if h["rubric"] == last_rubric_seen:
                continue
            last_rubric_seen = h["rubric"]
            r = h.get("rank", "")
            if r and r != seen_rank:
                rank_history.append([h["rubric"], r, RANK_LABEL.get(r, "")])
                seen_rank = r
        suppressed_in = explicit_suppressions.get(stem)
        canonization[f"Sancti/{stem}"] = {
            "added_in_rubric": first["rubric"],
            "first_mmdd": first["mmdd"],
            "first_officium": first["officium"],
            "rank_history": rank_history,
            "suppressed_in_rubric": suppressed_in,
            "last_live_rubric": last["rubric"],
            "kind": first["kind"],
        }
    return canonization, resolved_at


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--tabulae",
        type=Path,
        required=True,
        help="Path to Tabulae/Kalendaria directory",
    )
    ap.add_argument(
        "--out", type=Path, required=True, help="Output JSON file"
    )
    ap.add_argument(
        "--out-resolved",
        type=Path,
        default=None,
        help="Optional output JSON for the per-rubric resolved kalendaria",
    )
    args = ap.parse_args()
    if not args.tabulae.is_dir():
        print(f"error: {args.tabulae} is not a directory", file=sys.stderr)
        sys.exit(1)
    canonization, resolved_at = derive_canonization(args.tabulae)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(
        json.dumps(canonization, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
    print(
        f"wrote {len(canonization)} entries → {args.out} "
        f"({args.out.stat().st_size} bytes)",
        file=sys.stderr,
    )
    if args.out_resolved:
        args.out_resolved.parent.mkdir(parents=True, exist_ok=True)
        args.out_resolved.write_text(
            json.dumps(resolved_at, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )
        total = sum(len(k) for k in resolved_at.values())
        print(
            f"wrote {total} (rubric, mmdd) pairs → {args.out_resolved} "
            f"({args.out_resolved.stat().st_size} bytes)",
            file=sys.stderr,
        )


if __name__ == "__main__":
    main()
