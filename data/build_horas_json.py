#!/usr/bin/env python3
"""Extract the upstream Divinum Officium Breviary corpus into two
JSON bundles consumed by the Rust port:

    data/horas_latin.json   — keyed by `<dir>/<stem>` mirroring the
                              upstream filesystem layout. Each entry
                              is `{sections: {...}}` (a flat
                              section-name → body map). Covers:
                                Tempora/<stem>
                                Sancti/<stem>
                                Commune/<stem>
                                Ordinarium/<HourName>
                                Psalterium/Psalmi/<index>
                                Psalterium/Special/<index>
                              Plus the small singletons:
                                Psalterium/Invitatorium
                                Psalterium/Doxologies
                                Psalterium/Mariaant
                                Common/Prayers

    data/psalms_latin.json  — Psalterium/Psalmorum/Psalm{N}.txt
                              extracted to {N: {latin, latin_bea}}.
                              The upstream files are polyglot —
                              Latin Vulgate plus Latin-Bea (Pius XII)
                              as separate sections; we keep both so
                              the renderer can switch under the
                              `psalmvar` flag.

Runs against the current submodule:

    python3 data/build_horas_json.py

Mirrors the design of `data/build_missa_json.py` but flattened: the
breviary doesn't need the multi-rubric `[Rank]` annotation handling
that Mass needs (Breviary `[Rank]` lines are simpler and the runtime
already has the rubric).

The section grammar is identical to Mass: `[Section]` opens a block,
content runs until the next `[…]`. Conditional annotations like
`[Hymnus Vespera] (sed rubrica monastica)` are kept verbatim in the
section name so the Rust resolver can pick the right variant at
runtime — same convention `build_missa_json.py` uses.
"""

from __future__ import annotations
import json
import re
import sys
from pathlib import Path
from typing import Any

# Reuse the Ordo template walker for Ordinarium hour skeletons
# (`Vespera.txt`, `Laudes.txt`, …) — they share the `#Section`,
# `&macro`, `$prayer`, `(sed rubrica X)` grammar with the Mass
# `Ordo/Ordo*.txt` files.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from build_ordo_json import parse_template  # type: ignore  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
HORAS_LATIN = REPO / "vendor" / "divinum-officium" / "web" / "www" / "horas" / "Latin"
ORDINARIUM = REPO / "vendor" / "divinum-officium" / "web" / "www" / "horas" / "Ordinarium"

SECTION_RE = re.compile(r"^\[(?P<name>[^\]]+)\](?:\s*(?P<annotation>\(.*\)))?\s*$")


def parse_horas_file(text: str) -> dict[str, str]:
    """Split a horas-style file into `{section_name: body}`. The
    section_name carries any annotation `(rubrica X)`/`(tempore Y)`
    verbatim so the Rust resolver can pick the rubric-specific body —
    same convention as `build_missa_json.py`.

    Pre-section preamble (any content before the first `[Section]`
    header) is captured under the magic key `__preamble__`. The
    upstream `SetupString.pl::setupstring_parse_file` treats a
    leading `@Commune/CXX` line as a whole-file inheritance directive
    that merges the parent file's sections into this one — Saturday
    BVM `Commune/C10b` and several Sancti sub-files use this. The
    Rust resolver inspects `__preamble__` at runtime and follows the
    `@Path` redirect when the requested section is missing.
    """
    sections: dict[str, list[str]] = {}
    current: str | None = "__preamble__"
    sections[current] = []
    for raw in text.splitlines():
        m = SECTION_RE.match(raw.rstrip())
        if m is not None:
            base = m.group("name").strip()
            ann = (m.group("annotation") or "").strip()
            if ann:
                key = f"{base} {ann}"
            else:
                key = base
            # First occurrence wins (matches Perl SetupString first-
            # binding semantics).
            current = key if key not in sections else None
            if current is not None:
                sections[current] = []
            continue
        if current is not None:
            sections[current].append(raw)
    out = {k: "\n".join(v).strip() for k, v in sections.items()}
    # Drop the preamble key when empty so storage stays compact.
    if not out.get("__preamble__"):
        out.pop("__preamble__", None)
    return out


def walk_dir(root: Path, prefix: str, out: dict[str, dict]) -> int:
    """Walk a flat directory of `.txt` files, parse each as a horas
    file, store under `{prefix}/{stem}` in `out`. Returns count."""
    if not root.exists():
        return 0
    n = 0
    for p in sorted(root.glob("*.txt")):
        text = p.read_text(encoding="utf-8")
        sections = parse_horas_file(text)
        out[f"{prefix}/{p.stem}"] = {"sections": sections}
        n += 1
    return n


def parse_psalm_file(text: str) -> dict[str, str]:
    """Psalmorum/Psalm{N}.txt files are polyglot. We keep the
    [Latin] and [Latin-Bea] (Pius XII Vulgate revision) blocks; the
    latter is conditional under the `psalmvar` upstream flag.

    Some psalm files use bare `[Latin]` and `[Latin-Bea]` headers,
    but a few (the canticles) just dump body without a section
    header — we treat the whole file as `latin` in that case.
    """
    sections = parse_horas_file(text)
    out: dict[str, str] = {}
    if "Latin" in sections:
        out["latin"] = sections["Latin"]
    if "Latin-Bea" in sections:
        out["latin_bea"] = sections["Latin-Bea"]
    if not out:
        # Canticle / bare-body file — the whole text is the Latin body.
        out["latin"] = text.strip()
    return out


def main() -> None:
    if not HORAS_LATIN.exists():
        sys.stderr.write(f"FATAL: {HORAS_LATIN} not found. Run scripts/setup-divinum-officium.sh first.\n")
        sys.exit(1)

    horas: dict[str, dict] = {}

    # Per-day office files: Tempora / Sancti / Commune. Each is a
    # flat directory of `<stem>.txt` files.
    n_tempora = walk_dir(HORAS_LATIN / "Tempora", "Tempora", horas)
    n_sancti = walk_dir(HORAS_LATIN / "Sancti", "Sancti", horas)
    n_commune = walk_dir(HORAS_LATIN / "Commune", "Commune", horas)

    # Ordinarium hour skeletons. These use the same template grammar
    # as the Mass `Ordo/Ordo*.txt` files (`#Section`, `&macro`,
    # `$prayer`, `(sed rubrica X)`), NOT the `[Section]` grammar of
    # the per-day office files. We re-use `build_ordo_json.parse_
    # template` to emit a list of typed lines under a `template` key.
    n_ord = 0
    if ORDINARIUM.exists():
        for p in sorted(ORDINARIUM.glob("*.txt")):
            text = p.read_text(encoding="utf-8")
            template = parse_template(text)
            horas[f"Ordinarium/{p.stem}"] = {"template": template}
            n_ord += 1

    # Psalterium index files — `Psalmi major.txt`, `Psalmi
    # matutinum.txt`, `Psalmi minor.txt` — and Special/* (Major,
    # Matutinum, Minor, Prima, Preces).
    psalm_idx_dir = HORAS_LATIN / "Psalterium" / "Psalmi"
    psalm_special_dir = HORAS_LATIN / "Psalterium" / "Special"
    if psalm_idx_dir.exists():
        for p in sorted(psalm_idx_dir.glob("*.txt")):
            sections = parse_horas_file(p.read_text(encoding="utf-8"))
            # Use a flat slugified key — `Psalmi major.txt` →
            # `Psalterium/Psalmi/major`.
            slug = p.stem.replace("Psalmi ", "")
            horas[f"Psalterium/Psalmi/{slug}"] = {"sections": sections}
    if psalm_special_dir.exists():
        for p in sorted(psalm_special_dir.glob("*.txt")):
            sections = parse_horas_file(p.read_text(encoding="utf-8"))
            # `Major Special.txt` → `Psalterium/Special/Major`.
            slug = p.stem.replace(" Special", "")
            horas[f"Psalterium/Special/{slug}"] = {"sections": sections}

    # Singletons: Invitatorium, Doxologies, Mariaant, Common/Prayers.
    for rel in [
        "Psalterium/Invitatorium.txt",
        "Psalterium/Doxologies.txt",
        "Psalterium/Mariaant.txt",
        "Psalterium/Common/Prayers.txt",
        "Psalterium/Common/Rubricae.txt",
    ]:
        p = HORAS_LATIN / rel
        if not p.exists():
            continue
        key = rel[:-4]  # strip `.txt`
        sections = parse_horas_file(p.read_text(encoding="utf-8"))
        horas[key] = {"sections": sections}

    # Psalmorum/Psalm{N}.txt → separate JSON keyed by file stem
    # (`Psalm1`, `Psalm150`, plus split forms `Psalm17a`…).
    psalms: dict[str, dict] = {}
    psalmorum = HORAS_LATIN / "Psalterium" / "Psalmorum"
    if psalmorum.exists():
        for p in sorted(psalmorum.glob("Psalm*.txt")):
            psalms[p.stem] = parse_psalm_file(p.read_text(encoding="utf-8"))

    horas_path = REPO / "data" / "horas_latin.json"
    psalms_path = REPO / "data" / "psalms_latin.json"
    horas_path.write_text(json.dumps(horas, ensure_ascii=False, indent=1), encoding="utf-8")
    psalms_path.write_text(json.dumps(psalms, ensure_ascii=False, indent=1), encoding="utf-8")

    print(f"  tempora:    {n_tempora}")
    print(f"  sancti:     {n_sancti}")
    print(f"  commune:    {n_commune}")
    print(f"  ordinarium: {n_ord}")
    print(f"  psalms:     {len(psalms)}")
    print(f"  total horas keys: {len(horas)}")
    print(f"  written:    {horas_path} ({horas_path.stat().st_size:,} bytes)")
    print(f"  written:    {psalms_path} ({psalms_path.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
