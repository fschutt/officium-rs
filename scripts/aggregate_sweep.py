#!/usr/bin/env python3
"""Aggregate year-sweep manifests across many years and rubrics into a
single picture of where the Mass-side Rust port still diverges.

Usage:
    scripts/aggregate_sweep.py [--years FROM:TO] [--rubric SLUG ...]

By default reads every `target/regression/<rubric_slug>-<year>/manifest.json`
under the project root. Produces:

* Per-rubric pass-rate summary.
* Top inferred-source files (Rust missed, Perl used) ranked by hit count.
* Top winner-pair patterns (Rust → Perl) — surfaces wrong-winner clusters.
* Per-year breakdown so multi-year drift is visible.
* Day-level fail clusters: dates that fail in multiple years (cycle bugs).

This is the read-only investigation tool. It doesn't fix anything;
it just makes the systematic shape of the residual divergence
visible so we can decide what to port next.
"""

from __future__ import annotations

import argparse
import collections
import glob
import json
import os
import sys
from typing import Dict, List, Tuple

RUBRIC_SLUGS = [
    "Tridentine_1570",
    "Tridentine_1910",
    "Divino_Afflatu_1939",
    "Reduced_1955",
    "Rubrics_1960_1960",
]


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--years",
        default=None,
        help="Restrict to FROM:TO (inclusive). Default: all on disk.",
    )
    p.add_argument(
        "--rubric",
        action="append",
        default=None,
        help="Restrict to one or more rubric slugs (can be repeated).",
    )
    p.add_argument(
        "--root",
        default=os.path.join(os.path.dirname(__file__), ".."),
        help="Project root containing target/regression/. Default: parent of script.",
    )
    p.add_argument(
        "--top",
        type=int,
        default=20,
        help="How many entries to show in each top-N list.",
    )
    return p.parse_args()


def load_manifests(
    root: str,
    rubric_slugs: List[str],
    year_range: Tuple[int, int] | None,
) -> Dict[str, List[Tuple[int, dict]]]:
    """Returns {rubric_slug: [(year, manifest_dict), ...]}."""
    out: Dict[str, List[Tuple[int, dict]]] = {}
    for slug in rubric_slugs:
        rows: List[Tuple[int, dict]] = []
        for path in sorted(
            glob.glob(os.path.join(root, "target/regression", f"{slug}-*", "manifest.json"))
        ):
            year_str = os.path.basename(os.path.dirname(path)).rsplit("-", 1)[-1]
            try:
                year = int(year_str)
            except ValueError:
                continue
            if year_range and not (year_range[0] <= year <= year_range[1]):
                continue
            with open(path) as f:
                rows.append((year, json.load(f)))
        if rows:
            out[slug] = rows
    return out


def summarise_rubric(slug: str, manifests: List[Tuple[int, dict]], top_n: int) -> None:
    print()
    print("─" * 72)
    print(f"  RUBRIC: {slug}    {len(manifests)} year-manifests")
    print("─" * 72)

    days_total = 0
    days_passing = 0
    section_match = 0
    section_total = 0
    section_differ = 0
    panics = 0
    perl_failures = 0
    inferred_misses: collections.Counter[str] = collections.Counter()
    inferred_pairs: collections.Counter[str] = collections.Counter()
    fail_dates: collections.Counter[str] = collections.Counter()  # MM-DD across years
    per_year: List[Tuple[int, float, int, int]] = []

    for year, m in manifests:
        s = m["stats"]
        days_total += s["days_total"]
        days_passing += s["days_passing"]
        section_match += s["section_match"]
        section_total += s["section_total"]
        section_differ += s["section_differ"]
        panics += s.get("panics", 0)
        perl_failures += s.get("perl_failures", 0)
        per_year.append(
            (
                year,
                100.0 * s["days_passing"] / max(1, s["days_total"]),
                s["days_passing"],
                s["days_total"],
            )
        )

        for entry in m.get("inferred_top_misses", []):
            inferred_misses[entry["file_section"]] += entry["count"]
        for entry in m.get("inferred_top_pairs", []):
            inferred_pairs[entry["rust_to_perl"]] += entry["count"]

        # Day-level fail buckets — a date that fails across many years
        # is almost always a calendar-cycle bug, not a one-off.
        for d in m.get("days", []):
            if not d.get("winner_match", True) or any(
                sec.get("status") == "Differ" for sec in d.get("sections", [])
            ):
                fail_dates[d["date"][5:]] += 1  # strip year prefix

    pass_pct = 100.0 * days_passing / max(1, days_total)
    section_pct = 100.0 * section_match / max(1, section_total)
    print(
        f"  days passing:    {days_passing:>6}/{days_total:<6} ({pass_pct:5.2f}%)"
    )
    print(
        f"  section match:   {section_match:>6}/{section_total:<6} ({section_pct:5.2f}%)"
    )
    print(f"  section differ:  {section_differ}")
    print(f"  panics:          {panics}")
    print(f"  perl failures:   {perl_failures}")

    # Worst-performing years
    per_year.sort(key=lambda x: x[1])
    if per_year and per_year[0][1] < 100.0:
        print()
        print("  worst-performing years:")
        for year, pct, p, t in per_year[:5]:
            if pct >= 100.0:
                break
            print(f"    {year}: {p:>3}/{t} ({pct:5.2f}%)")

    if inferred_misses:
        print()
        print(f"  top {top_n} inferred Perl-source files (Rust missed):")
        for k, v in inferred_misses.most_common(top_n):
            print(f"    {v:>5}× {k}")

    if inferred_pairs:
        print()
        print(f"  top {top_n} winner-pair patterns (rust → perl):")
        for k, v in inferred_pairs.most_common(top_n):
            print(f"    {v:>5}× {k}")

    if fail_dates:
        print()
        print(f"  top {top_n} fail-dates (MM-DD, across years):")
        for k, v in fail_dates.most_common(top_n):
            print(f"    {v:>3}× {k}")


def main() -> int:
    args = parse_args()
    rubric_slugs = args.rubric if args.rubric else RUBRIC_SLUGS

    year_range = None
    if args.years:
        a, b = args.years.split(":")
        year_range = (int(a), int(b))

    manifests = load_manifests(args.root, rubric_slugs, year_range)
    if not manifests:
        print("No manifests found.", file=sys.stderr)
        return 1

    print()
    print(f"# Multi-rubric sweep aggregate")
    print(f"# range: {args.years or 'all'}    rubrics: {len(manifests)}")

    for slug in rubric_slugs:
        if slug in manifests:
            summarise_rubric(slug, manifests[slug], args.top)

    return 0


if __name__ == "__main__":
    sys.exit(main())
