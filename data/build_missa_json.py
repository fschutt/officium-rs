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

# Match `[SectionName]` and capture the optional rubric annotation
# trailing the closing bracket — `[Introitus] (communi Summorum
# Pontificum)`, `[Oratio] (rubrica 1960)`, `[Lectio] (tempore
# paschali)`. The annotation tells us which rubric layer the variant
# applies to; for the Tridentine 1570 baseline we filter post-1570
# variants.
SECTION_RE = re.compile(r"^\[([^\]]+)\](\s*\(([^)]*)\))?")
BASE_FILE_RE = re.compile(r"\.txt$")
# File-level inheritance: a `@Commune/C2` line at the top of a file
# (before any [Section]) means "inherit every missing section from
# Commune/C2". Captured as `parent` so the runtime resolver can
# follow.
PARENT_RE = re.compile(r"^@(\S+)\s*$")
# Conditional parent inherit: `(rubrica X)@Path` or `(predicate ...)@Path`.
# When the predicate matches the runtime version, the file's parent
# becomes <Path>; otherwise the unconditional parent (or no parent)
# applies. We capture (predicate, path) and let the runtime decide.
COND_PARENT_RE = re.compile(r"^\(([^)]+)\)\s*@(\S+)\s*$")
# Section-rubric annotations the Tridentine 1570 layer must EXCLUDE
# because they encode post-1570 reforms. Anything else (no annotation,
# `(rubrica tridentina)`, `(ad missam)`, `(tempore paschali)`, etc.)
# is kept.
EXCLUDED_ANNOTATIONS_1570 = (
    "communi summorum pontificum",
    "rubrica 1960",
    "rubrica 196",
    "rubrica 1955",
    "rubrica divino",
    "rubrica monastica",
    "rubrica cisterciensis",
    "rubrica ordo praedicatorum",
)


def is_excluded_annotation(annotation: str) -> bool:
    """True when this annotation marks a post-1570 rubric variant we
    should drop from the 1570 baseline corpus.

    Disjunctive predicates (`rubrica Divino aut rubrica Tridentina aut
    rubrica Monastica`) include 1570 if any disjunct mentions
    Tridentina/1570 — keep the section in that case so the consumer
    can pick up the body under 1570 mode.

    Matched case-insensitively: upstream sometimes writes `(rubrica
    divino et feria 3 …)` (lowercase d) and `(rubrica Divino …)`
    (capital D) for the same logical predicate."""
    if not annotation:
        return False
    a_lower = annotation.strip().lower()
    if "tridentina" in a_lower or "1570" in a_lower:
        return False
    return any(a_lower.startswith(needle) for needle in EXCLUDED_ANNOTATIONS_1570)


def parse_mass_file(text: str) -> dict:
    """Split a Mass file into a dict-of-sections plus a parsed [Rank]
    summary. The [Rank] body is stored separately because it isn't a
    proper section — it carries metadata (rank class / numeric rank /
    Commune ref) rather than printable Mass text.

    Tridentine-1570 annotation filter: when a section header has an
    annotation that matches a post-1570 rubric layer (`(communi
    Summorum Pontificum)`, `(rubrica 1960)`, etc.), we DROP its body
    entirely so the unannotated/Tridentine variant wins. When the
    bare section is missing AND the only variant is annotated, the
    section is left empty in the JSON — the consumer falls back via
    the file-level `parent` inherit (also captured below)."""
    sections: dict[str, list[str]] = {}
    annotations: dict[str, str] = {}
    current = None
    collecting = False
    parent: str | None = None
    parent_1570: str | None = None
    seen_section = False
    for raw in text.splitlines():
        m = SECTION_RE.match(raw.rstrip())
        if m is not None:
            seen_section = True
            base_name = m.group(1).strip()
            annotation = (m.group(3) or "").strip()
            # Treat seasonal annotations (`tempore X`) as a distinct
            # variant key so the runtime can pick the right body when
            # it knows the current season. Other annotations follow
            # the original first-wins / dropped-if-excluded rule.
            seasonal_variant = (
                annotation
                and annotation.lower().startswith("tempore ")
            )
            if seasonal_variant:
                current = f"{base_name} ({annotation})"
            else:
                current = base_name
            if current not in sections:
                sections[current] = []
                annotations[current] = annotation
                collecting = True
            else:
                # Later occurrence — first-occurrence-wins, drop body.
                collecting = False
            continue
        if current is not None and collecting:
            sections[current].append(raw)
            continue
        # Pre-section content: capture a leading `@Commune/X` as the
        # file-level inherit. Also recognise `(predicate)@Path` —
        # conditional parent inherit. Stop on first non-blank
        # non-conditional non-`@` line.
        if not seen_section:
            stripped = raw.strip()
            if stripped:
                cpm = COND_PARENT_RE.match(stripped)
                if cpm:
                    pred = cpm.group(1).strip().lower()
                    target = cpm.group(2)
                    # Tridentine variant — captured as parent_1570.
                    if "tridentina" in pred or "1570" in pred:
                        if parent_1570 is None:
                            parent_1570 = target
                    continue
                pm = PARENT_RE.match(stripped)
                if pm and parent is None:
                    parent = pm.group(1)

    out: dict = {}
    if "Officium" in sections:
        out["officium"] = " ".join(s.strip() for s in sections.pop("Officium")).strip() or None
    if "Rank" in sections:
        rank_lines = sections.pop("Rank")
        # Walk the body looking for variant blocks. Format:
        #   <default>;;Class;;Rank;;Commune
        #   (sed rubrica 1570 aut rubrica monastica)
        #   <1570 variant>;;Class;;Rank;;Commune
        #   (sed rubrica cisterciensis)
        #   <cist variant>;;Class;;Rank;;Commune
        # We capture the default (rank_num/rank/commune) plus the
        # rubrica-1570 variant (rank_num_1570) when one exists.
        default_parts = None
        variant_1570_parts = None
        current_label = None
        for raw in rank_lines:
            line = raw.strip()
            if not line:
                continue
            if line.startswith("(") and line.endswith(")"):
                inner = line[1:-1].strip().lower()
                # `(sed rubrica X aut rubrica Y)` — pick the first
                # rubrica name as the variant label.
                current_label = inner
                continue
            parts = [p.strip() for p in line.split(";;")]
            if current_label is None and default_parts is None:
                default_parts = parts
            elif (
                current_label
                and ("1570" in current_label or "tridentina" in current_label)
                and variant_1570_parts is None
            ):
                # Both "(sed rubrica 1570)" and "(sed rubrica
                # tridentina)" describe the Tridentine 1570 baseline.
                variant_1570_parts = parts
            current_label = None
        if default_parts:
            if not out.get("officium") and default_parts[0]:
                out["officium"] = default_parts[0]
            out["rank"] = default_parts[1] if len(default_parts) > 1 else None
            try:
                out["rank_num"] = (
                    float(default_parts[2]) if len(default_parts) > 2 and default_parts[2] else None
                )
            except ValueError:
                out["rank_num"] = None
            out["commune"] = default_parts[3] if len(default_parts) > 3 else None
        if variant_1570_parts:
            try:
                out["rank_num_1570"] = (
                    float(variant_1570_parts[2])
                    if len(variant_1570_parts) > 2 and variant_1570_parts[2]
                    else None
                )
            except ValueError:
                out["rank_num_1570"] = None
            # Capture the 1570 commune too — for Bibiana etc. it's
            # the same as default, but for some saints the commune
            # changes between rubrics.
            if len(variant_1570_parts) > 3 and variant_1570_parts[3]:
                out["commune_1570"] = variant_1570_parts[3]
    # Keep all remaining sections as joined strings so the renderer can
    # treat `\n` as a soft separator (matching the upstream convention).
    out["sections"] = {
        name: "\n".join(body).strip() for name, body in sections.items()
        if "\n".join(body).strip()
    }
    if parent:
        out["parent"] = parent
    if parent_1570:
        out["parent_1570"] = parent_1570
    # Sections that carry a post-1570 rubric annotation. Consumers
    # filtering for the Tridentine 1570 baseline should ignore these
    # IN COMMUNE-FALLBACK CONTEXT; explicit `@Commune/X` references
    # from a Sancti file still resolve through them.
    annotated = sorted(
        name for name, ann in annotations.items()
        if name in out.get("sections", {}) and is_excluded_annotation(ann)
    )
    if annotated:
        out["annotated_sections"] = annotated
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
            if parsed.get("sections") or parsed.get("officium") or parsed.get("parent"):
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
            if parsed.get("sections") or parsed.get("officium") or parsed.get("parent"):
                # Missa-side entry is authoritative; only fill in
                # from horas if missa didn't supply one.
                if key not in data:
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
