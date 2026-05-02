#!/usr/bin/env python3
"""
Preprocess the Divinum Officium Latin Mass corpus into a single
keyed JSON we ship as a build asset for /wip/missal.

Run once when the upstream Mass data changes:

    python3 md2json2/data/build_missa_json.py \
        --missa-latin   vendor/divinum-officium/web/www/missa/Latin \
        --horas-commune vendor/divinum-officium/web/www/horas/Latin/Commune \
        --out           md2json2/data/missa_latin.json

The `--missa-latin` argument MUST point at the same vendor tree the
Perl regression oracle uses (`scripts/do_render.sh` → `missa.pl`).
A different upstream (e.g. `divinum-officium-cgi-bin`) ships an older
file format with diverging `[Rank]` line bodies and would silently
poison every comparison.

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


def _annotation_matches_version(label: str, version: str) -> bool:
    """Mirror Perl's `vero()` predicate check: for each `rubrica X`
    predicate in `label`, the test is `$version =~ /X/i`. We extract
    each year-token (`19xx` or three-digit `19x`) and apply the same
    substring check against `version`.

    Conjunctions: `(sed rubrica X aut rubrica Y)` matches if EITHER
    X or Y substring-matches version. Negation (`nisi`) is not
    handled here — the parser drops `nisi`-only-clauses on the floor.
    A label that contains *no* year-token (e.g. just `tridentina` or
    `monastica`) returns False from this helper; predicate-name
    matching is handled by callers.
    """
    s = label.lower()
    v = version.lower()
    # Extract all year-tokens, longest first so we don't partial-match
    # e.g. "1960" inside "1962".
    import re as _re
    tokens = _re.findall(r"19\d{1,3}", s)
    for tok in tokens:
        if tok in v:
            return True
    return False


def _post_da_buckets(label: str) -> tuple[bool, bool]:
    """Return (matches_R55, matches_R60) for a `(rubrica ...)` body.

    Mirrors Perl `vero()` on `(rubrica X)` ⇒ `$version =~ /X/i`.
    R55 version = "Reduced - 1955"; R60 version = "Rubrics 1960 - 1960".

    Examples:
      `(rubrica 1955)`                — `/1955/` matches "Reduced - 1955"
                                         but NOT "Rubrics 1960 - 1960".
                                         ⇒ R55-only.
      `(rubrica 196)`                 — `/196/` substring-matches
                                         "Rubrics 1960" but NOT
                                         "Reduced - 1955". ⇒ R60-only.
      `(rubrica 1962)`                — `/1962/` doesn't match either
                                         version string. ⇒ neither.
      `(rubrica 196 aut rubrica 1955)` — R55 + R60.

    Predicate-name annotations (`tridentina`, `monastica`,
    `cisterciensis`) without year tokens fall outside this helper —
    handled by `_t1570_bucket` / `_t1910_bucket`.
    """
    return (
        _annotation_matches_version(label, "Reduced - 1955"),
        _annotation_matches_version(label, "Rubrics 1960 - 1960"),
    )


def _t1570_bucket(label: str) -> bool:
    """Return True if a `(sed rubrica …)` annotation applies to T1570.

    The Perl SetupString predicates work like:
      * `tridentina` ⇒ `$version =~ /Trident/`  (matches T1570 *and*
                                                  T1910; also Monastic
                                                  Tridentinum 1617).
      * `1570`       ⇒ `$version =~ /1570/`     (matches only T1570).

    A bare `1570`-only annotation is T1570-specific; a `tridentina`
    annotation also fires for T1910 (handled in `_t1910_bucket`).
    """
    s = label.lower()
    return "1570" in s or "tridentina" in s


def _t1910_bucket(label: str) -> bool:
    """Return True if a `(rubrica …)` annotation applies to T1910
    (Perl version "Tridentine - 1910").

    T1910 matches Perl predicates that test against the version string
    `"Tridentine - 1910"`:
      * `tridentina`  — Perl `/Trident/` ⇒ TRUE
      * literal `1910` — `/1910/` ⇒ TRUE
      * literal `1570/1888/1906` — those years don't appear in
                       "Tridentine - 1910", so FALSE.
      * post-DA tokens — also FALSE.

    So `(sed rubrica tridentina)` activates the variant under T1910,
    but `(rubrica 1906 aut rubrica cisterciensis)` does NOT — T1910
    keeps the bare default in that case (Tempora/Pent02-5o stays
    Duplex majus 4.01, doesn't elevate to Duplex I classis 6.5).
    """
    s = label.lower()
    return "tridentina" in s or "1910" in s


def _is_t1910_or_post_da_rubric(label: str) -> bool:
    """Compatibility shim — true if any of T1910 / R55 / R60 matches.
    Used for deciding whether to emit a second-header [Rank] body."""
    a, b = _post_da_buckets(label)
    return a or b or _t1910_bucket(label)


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
                # Later occurrence. For [Rank] specifically, surface an
                # annotated second header carrying a *post-DA* variant
                # — `[Rank] (rubrica 196 aut rubrica 1955)`,
                # `[Rank] (rubrica 196)`, etc. — so the walker below
                # can read it as a 1955+ override.
                #
                # `[Rank] (rubrica 1570)` second headers are
                # intentionally NOT captured: they replicate via the
                # already-handled inline `(sed rubrica 1570)` form on
                # other files, and ingesting them here would inject a
                # 1570-variant rank where Perl actually keeps the
                # default body for non-1570 rubrics. (The Perl
                # SetupString re-opens the second header under T1570
                # and discards the first body via the
                # section-conditional branch — but then in non-T1570
                # the first body is what survives. Our two-bucket
                # `rank_num` / `rank_num_1955` matches that for
                # post-DA only, so we keep 1570 out of the second-
                # header path.)
                is_post_da_variant = (
                    base_name == "Rank"
                    and annotation
                    and _is_t1910_or_post_da_rubric(annotation)
                )
                if is_post_da_variant:
                    sections[current].append(f"({annotation})")
                    collecting = True
                else:
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
        #   (rubrica 196 aut rubrica 1955)
        #   <1955+ variant>;;Class;;Rank;;Commune
        # Three buckets:
        #   default → rank_num / commune (applies when no other
        #     variant matches)
        #   1570/tridentina → rank_num_1570 / commune_1570
        #   196/1955 → rank_num_1955 / commune_1955 (also surfaces as
        #     the "post-DA" variant; a few files use just `(rubrica 196)`
        #     which is 1960-only — we still bucket those here)
        default_parts = None
        variant_1570_parts = None
        variant_1906_parts = None
        variant_1955_parts = None
        variant_1960_parts = None
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
            elif current_label:
                # Each rubric is bucketed independently — a single
                # annotation line can populate multiple variants when
                # its predicate is a disjunction or a generic
                # `tridentina` (which matches T1570 + T1910 alike).
                if _t1570_bucket(current_label) and variant_1570_parts is None:
                    variant_1570_parts = parts
                if _t1910_bucket(current_label) and variant_1906_parts is None:
                    variant_1906_parts = parts
                m55, m60 = _post_da_buckets(current_label)
                if m55 and variant_1955_parts is None:
                    variant_1955_parts = parts
                if m60 and variant_1960_parts is None:
                    variant_1960_parts = parts
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
        if variant_1906_parts:
            if len(variant_1906_parts) > 0 and variant_1906_parts[0]:
                out["officium_1906"] = variant_1906_parts[0]
            if len(variant_1906_parts) > 1 and variant_1906_parts[1]:
                out["rank_1906"] = variant_1906_parts[1]
            try:
                out["rank_num_1906"] = (
                    float(variant_1906_parts[2])
                    if len(variant_1906_parts) > 2 and variant_1906_parts[2]
                    else None
                )
            except ValueError:
                out["rank_num_1906"] = None
            if len(variant_1906_parts) > 3 and variant_1906_parts[3]:
                out["commune_1906"] = variant_1906_parts[3]
        if variant_1955_parts:
            if len(variant_1955_parts) > 0 and variant_1955_parts[0]:
                out["officium_1955"] = variant_1955_parts[0]
            if len(variant_1955_parts) > 1 and variant_1955_parts[1]:
                out["rank_1955"] = variant_1955_parts[1]
            try:
                out["rank_num_1955"] = (
                    float(variant_1955_parts[2])
                    if len(variant_1955_parts) > 2 and variant_1955_parts[2]
                    else None
                )
            except ValueError:
                out["rank_num_1955"] = None
            if len(variant_1955_parts) > 3 and variant_1955_parts[3]:
                out["commune_1955"] = variant_1955_parts[3]
        if variant_1960_parts:
            if len(variant_1960_parts) > 0 and variant_1960_parts[0]:
                out["officium_1960"] = variant_1960_parts[0]
            if len(variant_1960_parts) > 1 and variant_1960_parts[1]:
                out["rank_1960"] = variant_1960_parts[1]
            try:
                out["rank_num_1960"] = (
                    float(variant_1960_parts[2])
                    if len(variant_1960_parts) > 2 and variant_1960_parts[2]
                    else None
                )
            except ValueError:
                out["rank_num_1960"] = None
            if len(variant_1960_parts) > 3 and variant_1960_parts[3]:
                out["commune_1960"] = variant_1960_parts[3]
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
    # Sections that carry a post-1570 rubric annotation. Each entry
    # is a [section, annotation] pair so the consumer can re-evaluate
    # the annotation under the active rubric — `(communi Summorum
    # Pontificum)` is post-1570 baseline (drop) but TRUE under
    # R55/R60 (where Perl's `summorum pontificum` predicate
    # `/194[2-9]|195[45]|196/i` matches the version string), so
    # those sections should fire as winner-bodies under those
    # later rubrics rather than being skipped to commune fallback.
    annotated_pairs = sorted(
        (name, ann) for name, ann in annotations.items()
        if name in out.get("sections", {}) and is_excluded_annotation(ann)
    )
    if annotated_pairs:
        out["annotated_sections"] = [name for name, _ in annotated_pairs]
        out["annotated_section_meta"] = {
            name: ann for name, ann in annotated_pairs
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
            if parsed.get("sections") or parsed.get("officium") or parsed.get("parent"):
                out[key] = parsed
    return out


def _harvest_horas_dir(data: dict, root: Path, key_prefix: str) -> int:
    """Scan a horas-side subtree (`Sancti`/`Tempora`) and pull files
    whose missa equivalent is missing. Mirrors Perl
    `SetupString.pl::checkfile`, which falls back from `missa/Latin`
    to `horas/Latin` for any non-Commune file the missa tree lacks.

    Used to capture entries like `Sancti/10-DP` (Solemnitas Rosarii
    on the first Sunday of October — referenced by the Transfer
    table under T1888/T1910/Cisterciensis) which only ship under the
    horas tree."""
    added = 0
    if not root.is_dir():
        return 0
    for path in sorted(root.iterdir()):
        if not BASE_FILE_RE.search(path.name):
            continue
        key = f"{key_prefix}/{path.stem}"
        if key in data:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            text = path.read_text(encoding="latin-1")
        parsed = parse_mass_file(text)
        if parsed.get("sections") or parsed.get("officium") or parsed.get("parent"):
            data[key] = parsed
            added += 1
    return added


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--missa-latin", required=True, type=Path,
                    help="Path to <upstream>/web/www/missa/Latin")
    ap.add_argument("--horas-commune", type=Path, default=None,
                    help="Path to <upstream>/web/www/horas/Latin/Commune "
                         "(upstream stores Common files here, shared between "
                         "Office and Mass; @Commune/Cxx refs resolve here)")
    ap.add_argument("--horas-latin", type=Path, default=None,
                    help="Path to <upstream>/web/www/horas/Latin "
                         "(parent of the horas Sancti/Tempora subtrees). "
                         "When given, files present here but absent from "
                         "missa/Latin/Sancti or missa/Latin/Tempora are "
                         "added under their Sancti/<stem> or Tempora/<stem> "
                         "key — matches Perl SetupString::checkfile cascade.")
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
    if args.horas_latin is not None:
        if not args.horas_latin.is_dir():
            print(f"error: {args.horas_latin} is not a directory", file=sys.stderr)
            sys.exit(1)
        added_s = _harvest_horas_dir(data, args.horas_latin / "Sancti", "Sancti")
        added_t = _harvest_horas_dir(data, args.horas_latin / "Tempora", "Tempora")
        if added_s or added_t:
            print(f"   horas-latin fallback: +{added_s} Sancti, +{added_t} Tempora")
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
