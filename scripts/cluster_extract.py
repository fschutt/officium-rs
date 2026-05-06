#!/usr/bin/env python3
"""Extract per-cluster failing days from per-year regression manifests.

For each named cluster (winner-pair pattern + rubric), walk every
`target/regression/<slug>-<year>/manifest.json`, collect the
specific dates that match the cluster's pattern, and write them to
`target/regression/clusters/<cluster_name>.txt` as one
`YYYY-MM-DD` per line.

This freezes the residual set so we can fix one cluster at a time
and verify just its days, instead of re-running 100 years × 5
rubrics after every code change. After all clusters are closed,
we do ONE final 100-year sweep to confirm nothing else broke.

Usage:
    scripts/cluster_extract.py            # dump every cluster
    scripts/cluster_extract.py NAME ...   # dump just the named ones

Output:
    target/regression/clusters/<name>.txt
"""

from __future__ import annotations

import glob
import json
import os
import sys
from typing import Callable, Dict, List, Tuple

# ───────────────────────────────────────────────────────────────────
# Cluster registry — drawn from docs/REGRESSION_RESIDUALS.md.
#
# Each cluster is a 3-tuple:
#   (rubric_slug, predicate, description)
#
# `predicate(rust_winner, perl_winner_inferred, day) -> bool`
# is called for each day-record in the manifest; a True return
# claims that day for the cluster.
#
# `perl_winner_inferred` is the top inferred-source Perl file the
# regression harness deduced for that day; pulled from the day's
# `inferred_pairs` if present in the manifest, else `None`.
# ───────────────────────────────────────────────────────────────────

ClusterPred = Callable[[str, str, dict], bool]


def _winner_pair(rust: str, perl: str) -> ClusterPred:
    return lambda r, p, d: r == rust and p == perl


def _winner_pairs(*pairs: Tuple[str, str]) -> ClusterPred:
    s = set(pairs)
    return lambda r, p, d: (r, p) in s


def _rust_in(*winners: str) -> ClusterPred:
    s = set(winners)
    return lambda r, p, d: r in s


CLUSTERS: Dict[str, Tuple[str, ClusterPred, str]] = {
    "DA_EpiphanySunday": (
        "Divino_Afflatu_1939",
        _winner_pairs(
            ("Tempora/Epi1-0", "Sancti/01-06"),
            ("Tempora/Epi1-0", "Sancti/01-13"),
        ),
        "Sunday-after-Epiphany displaced by Holy Family / Baptism — DA",
    ),
    "R60_RogationDays": (
        "Rubrics_1960_1960",
        _winner_pairs(
            ("Tempora/Pasc6-1", "Tempora/Pasc5-4"),
            ("Tempora/Pasc6-2", "Tempora/Pasc5-4"),
            ("Tempora/Pasc6-3", "Tempora/Pasc5-4"),
            ("Tempora/Pasc5-5", "Tempora/Pasc5-4"),
        ),
        "Rogation Days reuse Pasc5-4 readings under R60",
    ),
    "DA_SeptEmbersCross": (
        "Divino_Afflatu_1939",
        _winner_pairs(
            ("Tempora/Pent14-0", "Sancti/09-14"),
            ("Tempora/Pent15-0", "Sancti/09-14"),
            ("Tempora/Pent16-0", "Sancti/09-14"),
            ("Tempora/Pent17-0", "Sancti/09-14"),
            ("Tempora/Pent18-0", "Sancti/09-14"),
        ),
        "September Embers / Cross outranks Pent Sunday — DA",
    ),
    "R55_SeptEmbersCross": (
        "Reduced_1955",
        _winner_pairs(
            ("Tempora/Pent14-0", "Sancti/09-14"),
            ("Tempora/Pent15-0", "Sancti/09-14"),
            ("Tempora/Pent16-0", "Sancti/09-14"),
            ("Tempora/Pent17-0", "Sancti/09-14"),
            ("Tempora/Pent18-0", "Sancti/09-14"),
        ),
        "September Embers / Cross outranks Pent Sunday — R55",
    ),
    "R55_TrinityFriday": (
        "Reduced_1955",
        _winner_pair("Tempora/Pent01-5", "Tempora/Pent01-0"),
        "Pent01-5 falls back to Pent01-0 body — R55",
    ),
    "T1910_Cathedra_Matthias": (
        "Tridentine_1910",
        _winner_pairs(
            ("Sancti/02-22", "Sancti/02-24"),
            ("Sancti/02-23r", "Sancti/02-24"),
        ),
        "Cathedra Petri Romae + Vigil-of-Matthias cluster — T1910",
    ),
    "T1910_Annunciation": (
        "Tridentine_1910",
        _winner_pair("Sancti/03-25", "Tempora/Quad4-0"),
        "Annunciation in Lent collision — T1910",
    ),
    "Pent19_23_SelfRef_R55": (
        "Reduced_1955",
        _winner_pairs(
            ("Tempora/Pent19-0", "Tempora/Pent19-0"),
            ("Tempora/Pent20-0", "Tempora/Pent20-0"),
            ("Tempora/Pent21-0", "Tempora/Pent21-0"),
            ("Tempora/Pent22-0", "Tempora/Pent22-0"),
            ("Tempora/Pent23-0", "Tempora/Pent23-0"),
        ),
        "Pent19-23 same-file content diverges (rubrica variant?) — R55",
    ),
    "Pent19_23_SelfRef_R60": (
        "Rubrics_1960_1960",
        _winner_pairs(
            ("Tempora/Pent19-0", "Tempora/Pent19-0"),
            ("Tempora/Pent20-0", "Tempora/Pent20-0"),
            ("Tempora/Pent21-0", "Tempora/Pent21-0"),
            ("Tempora/Pent22-0", "Tempora/Pent22-0"),
            ("Tempora/Pent23-0", "Tempora/Pent23-0"),
        ),
        "Pent19-23 same-file content diverges (rubrica 1960) — R60",
    ),
    "Quadp_Quad_Commune_C4a": (
        # Cross-rubric — the one cluster that pools across T1910+DA+R55.
        # We extract per-rubric so the verify-step can handle the
        # cross-rubric fan-out as three sub-clusters.
        "Tridentine_1910",
        _winner_pairs(
            ("Tempora/Quadp1-2", "Commune/C2-1"),
            ("Tempora/Quadp1-3", "Sancti/02-12"),
            ("Tempora/Quadp1-4", "Commune/C4a"),
            ("Tempora/Quadp2-1", "Commune/C4a"),
            ("Tempora/Quadp2-3", "Sancti/02-12"),
            ("Tempora/Quadp3-1", "Commune/C4a"),
            ("Tempora/Quadp3-6", "Commune/C4a"),
            ("Tempora/Quad1-1", "Commune/C4a"),
            ("Tempora/Quad2-1", "Commune/C4a"),
        ),
        "Septuagesima/Quadragesima ferias → Commune/C4a — T1910",
    ),
    "R55_Propaganda": (
        "Reduced_1955",
        _winner_pairs(
            ("Tempora/Pent22-0", "Commune/Propaganda"),
            ("Tempora/Pent21-0", "Commune/Propaganda"),
            ("Tempora/Pent19-0", "Commune/Propaganda"),
        ),
        "Diocese-conditional Commune/Propaganda — R55",
    ),
    "T1570_12_08o": (
        "Tridentine_1570",
        _winner_pair("Sancti/12-08o", "Commune/C11"),
        "(closed in c093c2f — was harness-side setupstring cache bug)",
    ),
}


def winner_pair_for_day(d: dict) -> Tuple[str, str | None]:
    """Best-effort `(rust_winner, perl_inferred)` for a day-record.

    `winner_rust` is straight from the manifest. `perl_inferred`
    must be reconstructed from the per-section inferred-source
    hits we recorded — pick the most-frequent file across all
    Differ/RustBlank cells. Returns `None` for perl when no
    differing section had an inferred source.
    """
    rust = d.get("winner_rust", "")
    return rust, d.get("perl_inferred", None)


def find_manifests(root: str, slug: str) -> List[Tuple[int, str]]:
    out: List[Tuple[int, str]] = []
    for path in glob.glob(os.path.join(root, "target/regression", f"{slug}-*", "manifest.json")):
        year_str = os.path.basename(os.path.dirname(path)).rsplit("-", 1)[-1]
        try:
            out.append((int(year_str), path))
        except ValueError:
            pass
    out.sort()
    return out


def perl_inferred_for_day(day: dict) -> str | None:
    """Reconstruct the day's dominant Perl-inferred source. Manifest
    days carry per-section section/category but not the inferred
    pairs directly; fall back to scanning for the manifest's
    `inferred_top_pairs` filtered by `winner_rust`.
    """
    return None  # filled in by caller using manifest-level top-pairs


def extract_cluster(
    root: str,
    rubric_slug: str,
    pred: ClusterPred,
) -> List[Tuple[int, int, int]]:
    """Return sorted list of (year, mm, dd) tuples that satisfy `pred`."""
    out: List[Tuple[int, int, int]] = []
    for year, path in find_manifests(root, rubric_slug):
        with open(path) as f:
            m = json.load(f)
        # Build a per-day perl_inferred lookup from manifest-level
        # top-pairs. The manifest stores `inferred_top_pairs` =
        # `[{rust_to_perl: "Rust → Perl", count: N}, ...]`. For each
        # day, we'd need the per-day winning pair, which isn't
        # serialised. As a practical workaround: a day is claimed
        # by the cluster iff its `winner_rust` matches one of the
        # rust-side strings in any pair the predicate would accept,
        # AND any Differ section's first inferred-source `top.file`
        # matches one of the perl-side strings.
        for day_rec in m.get("days", []):
            rust = day_rec.get("winner_rust", "")
            # Reconstruct perl from a cluster-pair lookup:
            # we don't have per-day inferred, so fall through to
            # top-level `inferred_top_pairs` matched by rust prefix.
            day_pair = None
            for entry in m.get("inferred_top_pairs", []):
                pair_str = entry.get("rust_to_perl", "")
                rs, _, ps = pair_str.partition(" -> ")
                if rs == rust:
                    day_pair = (rs, ps)
                    break
            perl = day_pair[1] if day_pair else ""
            if pred(rust, perl, day_rec):
                # Only claim days that ACTUALLY differ — winner_match
                # is True doesn't necessarily mean pass; check sections.
                differs = any(
                    s.get("status") in ("differ", "rust_blank")
                    for s in day_rec.get("sections", [])
                )
                if differs:
                    date_str = day_rec["date"]  # YYYY-MM-DD
                    y, mm, dd = date_str.split("-")
                    out.append((int(y), int(mm), int(dd)))
    return sorted(set(out))


def main() -> int:
    project_root = os.path.join(os.path.dirname(__file__), "..")
    out_dir = os.path.join(project_root, "target/regression/clusters")
    os.makedirs(out_dir, exist_ok=True)

    selected = sys.argv[1:] or list(CLUSTERS.keys())
    for name in selected:
        if name not in CLUSTERS:
            print(f"unknown cluster: {name}", file=sys.stderr)
            continue
        rubric_slug, pred, desc = CLUSTERS[name]
        days = extract_cluster(project_root, rubric_slug, pred)
        out_path = os.path.join(out_dir, f"{name}.txt")
        with open(out_path, "w") as f:
            f.write(f"# cluster: {name}\n")
            f.write(f"# rubric: {rubric_slug}\n")
            f.write(f"# desc:   {desc}\n")
            f.write(f"# count:  {len(days)}\n")
            for y, mm, dd in days:
                f.write(f"{y:04}-{mm:02}-{dd:02}\n")
        print(f"  {name:30}  {len(days):3} days  → {out_path}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
