#!/usr/bin/env python3
"""Extract Ordo / Prayers / Prefationes from the upstream Divinum
Officium corpus into `data/ordo_latin.json`.

Mirrors what the Perl `propers.pl::specials()` walker reads at runtime.
We preserve enough structure to drive a lookup-driven Rust renderer
(see `src/ordo.rs`), so the demo no longer has any Latin Mass content
hardcoded in JS.

Output JSON shape:

    {
      "templates": {
        "Ordo":    [ {kind, ...}, ... ],
        "Ordo67":  [ ... ],
        "OrdoN":   [ ... ],
        "OrdoA":   [ ... ],
        "OrdoM":   [ ... ],
        "OrdoOP":  [ ... ],
        "OrdoS":   [ ... ]
      },
      "prayers":   { "<name>": "<body>", ... },
      "prefaces":  { "<name>": "<body>", ... }
    }

Each template is a list of `OrdoLine`s in order. Conditional `!*FLAG`
blocks are flattened: every line carries its `guard` (None for
unconditional). The renderer applies `(solemn, defunctorum)` mode at
runtime.

Line kinds (matching `crate::data_types::OrdoLineKind`):
  - "plain"      : { body }
  - "spoken"     : { role, body }    (V./R./S./M./D./C./J.)
  - "rubric"     : { body, level }   (! → 1, !! → 2, !!! → 3, !x! → x)
  - "section"    : { label }         (# Heading)
  - "macro"      : { name }          (&MacroName, looks up `prayers`)
  - "proper"     : { name }          (&propername, looks up the day's MassPropers)
  - "hook"       : { name }          (!&hookname, runtime callback)
  - "blank"      : {}                (terminates conditional block)

Build-script invocation:

    python3 data/build_ordo_json.py

(or run via the cargo build.rs transcode step).
"""

from __future__ import annotations
import json
import re
import sys
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parent.parent
ORDO_DIR = REPO / "vendor" / "divinum-officium" / "web" / "www" / "missa" / "Latin" / "Ordo"

# Names of the per-cursus Ordo files. Selected by `Cmissa.pl:52-53` in
# upstream — version → Ordo file. Tridentine 1570/1888/1906/DA/R55/R60
# all use plain "Ordo"; "Ordo67" is the 1967 reform; "OrdoN" is Novus
# Ordo; OrdoA/OrdoM/OrdoS/OrdoOP are non-Roman uses.
TEMPLATE_FILES = ["Ordo", "Ordo67", "OrdoN", "OrdoA", "OrdoM", "OrdoOP", "OrdoS"]

# `&Macro` references that point at proper-insertion sites. Everything
# else is a static-text macro resolved against `prayers`.
PROPER_MACROS = {
    "introitus", "collect", "lectio", "graduale", "evangelium",
    "offertorium", "secreta", "prefatio", "communicantes", "hancigitur",
    "Communio_Populi", "communio", "postcommunio", "itemissaest",
    "Ultimaev",
    # not strictly a proper insert — pulls today's Asperges-or-Vidi-aquam:
    "Vidiaquam",
    # `&DominusVobiscum` is a static dialog — handled as a normal macro.
}

GUARD_PATTERN = re.compile(r"^!\*([A-Za-z]+)$")
HOOK_PATTERN = re.compile(r"^!\*?&([A-Za-z_]+)\s*$")
ROLE_PATTERN = re.compile(r"^([VvRrSsMmCcDdJj])\.\s*(.*)$")


def parse_template(text: str) -> list[dict[str, Any]]:
    """Walk Ordo.txt-style text and emit a flat list of OrdoLine dicts.

    Conditional `!*FLAG` blocks set a guard that propagates to every
    line until the next blank line (matching Perl `specials()`'s
    `while ($t[$tind] !~ /^\\s*$/) { $tind++; }`).
    """
    out: list[dict[str, Any]] = []
    current_guard: str | None = None

    for raw in text.split("\n"):
        line = raw.rstrip()
        stripped = line.strip()

        # Blank line — terminates any conditional block.
        if not stripped:
            current_guard = None
            out.append({"kind": "blank"})
            continue

        # Section-header (`# Heading`).
        if stripped.startswith("#"):
            label = stripped.lstrip("# ").rstrip()
            entry = {"kind": "section", "label": label}
            if current_guard:
                entry["guard"] = current_guard
            out.append(entry)
            continue

        # Conditional flag — opens a guarded block. Don't emit a line
        # for the flag itself; just set the guard that will tag every
        # subsequent line until the next blank line.
        #
        # Shapes:
        #   `!*D`, `!*R`, `!*S`, `!*nD`, `!*RnD`, `!*SnD`
        #     → flag-guard. Renderer evaluates against (solemn, defunctorum).
        #   `!*&hookname`
        #     → hook-guard. Renderer calls the hook; if it returns true,
        #       skip the block. Encoded as `&hookname` so the renderer can
        #       distinguish it from flag-guards by the leading ampersand.
        #
        # The Perl source `propers.pl::specials()` accepts at most one
        # of each (a hook *and* a flag), but in practice the upstream
        # Ordo files never combine them. We treat them as mutually
        # exclusive — first one wins.
        if stripped.startswith("!*"):
            tail = stripped[2:].strip()
            hook_match = re.match(r"^&([A-Za-z_]+)\s*(.*)$", tail)
            if hook_match:
                hook_name = hook_match.group(1)
                rest = hook_match.group(2).strip()
                # `!*&hookname` is dual-purpose in Perl
                # `propers.pl::specials()`: the hook is `eval`'d, which
                # (a) runs the function's side effect — typically `push
                # @s, "!omit. <thing>"` — AND (b) returns the value
                # used as the block's skip flag. The side effect lands
                # in `@s` regardless of whether the block is then
                # skipped. We model this as: emit an *unguarded* hook
                # line for the side effect, then open the hook-guard
                # for the block.
                out.append({"kind": "hook", "name": hook_name})
                current_guard = f"&{hook_name}"
                if rest:
                    out.append({"kind": "rubric", "body": rest, "level": 1, "guard": current_guard})
                continue
            flag_match = re.match(r"^([A-Za-z]+)$", tail)
            if flag_match:
                current_guard = flag_match.group(1)
                continue
            # Fall-through — unrecognized `!*` shape; treat as rubric.
            current_guard = None

        # Hook (single-line `!&name`) — runtime callback.
        if stripped.startswith("!&"):
            name = stripped[2:].strip()
            entry = {"kind": "hook", "name": name}
            if current_guard:
                entry["guard"] = current_guard
            out.append(entry)
            continue

        # Rubric levels: `!!!text` (level 3, large), `!!text` (level 2,
        # small caps), `!x!text` (level x, omitted-comment style),
        # `!text` (level 1, italic).
        if stripped.startswith("!!!"):
            entry = {"kind": "rubric", "body": stripped[3:].strip(), "level": 3}
        elif stripped.startswith("!x!!"):
            entry = {"kind": "rubric", "body": stripped[4:].strip(), "level": 22}
        elif stripped.startswith("!x!"):
            entry = {"kind": "rubric", "body": stripped[3:].strip(), "level": 21}
        elif stripped.startswith("!!"):
            entry = {"kind": "rubric", "body": stripped[2:].strip(), "level": 2}
        elif stripped.startswith("!"):
            entry = {"kind": "rubric", "body": stripped[1:].strip(), "level": 1}
        elif stripped.startswith("&"):
            # Macro or proper insertion.
            name = stripped[1:].split()[0]
            if name in PROPER_MACROS:
                entry = {"kind": "proper", "name": name}
            else:
                entry = {"kind": "macro", "name": name}
        else:
            # Spoken (role-prefixed) or plain text.
            role_match = ROLE_PATTERN.match(stripped)
            if role_match:
                entry = {
                    "kind": "spoken",
                    "role": role_match.group(1).upper(),
                    "body": role_match.group(2),
                }
            else:
                entry = {"kind": "plain", "body": stripped}

        if current_guard:
            entry["guard"] = current_guard
        out.append(entry)

    return out


def parse_prayers(text: str) -> dict[str, str]:
    """Read `[Header]` ... body ... blocks into `{name: body}`."""
    out: dict[str, str] = {}
    current_name: str | None = None
    current_body: list[str] = []
    for line in text.split("\n"):
        m = re.match(r"^\[(.+?)\]\s*$", line)
        if m:
            if current_name is not None:
                out[current_name] = "\n".join(current_body).strip()
            current_name = m.group(1)
            current_body = []
        else:
            current_body.append(line)
    if current_name is not None:
        out[current_name] = "\n".join(current_body).strip()
    return out


def main() -> None:
    if not ORDO_DIR.exists():
        sys.stderr.write(f"FATAL: {ORDO_DIR} not found. Run scripts/setup-divinum-officium.sh first.\n")
        sys.exit(1)

    out: dict[str, Any] = {"templates": {}, "prayers": {}, "prefaces": {}}

    for name in TEMPLATE_FILES:
        path = ORDO_DIR / f"{name}.txt"
        if not path.exists():
            sys.stderr.write(f"WARN: {path} missing — skipping\n")
            continue
        out["templates"][name] = parse_template(path.read_text(encoding="utf-8"))

    prayers_path = ORDO_DIR / "Prayers.txt"
    if prayers_path.exists():
        out["prayers"] = parse_prayers(prayers_path.read_text(encoding="utf-8"))

    prefaces_path = ORDO_DIR / "Prefationes.txt"
    if prefaces_path.exists():
        out["prefaces"] = parse_prayers(prefaces_path.read_text(encoding="utf-8"))

    target = REPO / "data" / "ordo_latin.json"
    target.write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")

    print(f"  templates: {', '.join(out['templates'].keys())}")
    print(f"  prayers:   {len(out['prayers'])}")
    print(f"  prefaces:  {len(out['prefaces'])}")
    print(f"  written:   {target}")


if __name__ == "__main__":
    main()
