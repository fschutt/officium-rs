# Hardcode audit + refactor plan

Inventory of place-by-place hardcoded values in `officium-rs` Rust + JS,
mapped to the upstream Perl's data-driven equivalent. Goal: replace
each hack with a lookup against vendored `Tabulae/` / `Ordo/` data so
new feasts / rubric variants land as data-only changes upstream.

## Survey method

`grep -rn` across `src/` and `demo/` for:
- Hardcoded `month==`, `day==`, `dow==` branches.
- `officium.contains("...")` substring feast detection.
- Hardcoded `Sancti/`/`Tempora/`/`Commune/` file stems in non-test code.
- Float-literal rank values returned from helper functions.
- Per-rubric branches in match arms that should consult a table.
- The Mass Ordinary text living in `demo/ordo.js`.

Then for each cluster: identify the upstream Perl file that drives it
via lookup, so the refactor target is concrete.

---

## A. Mass Ordinary — currently hardcoded in JavaScript

### Where it lives now
`demo/ordo.js` (332 lines, all literal Latin) holds the Tridentine Mass
Ordinary as a JS array of `{kind, role, body, header, ...}` entries.
Includes Confiteor (collapsed), Kyrie, Gloria, Credo, Sanctus, Pater
Noster, Agnus Dei, Ite Missa Est, Last Gospel, etc.

### Why it's wrong
1. Drifts from upstream as the corpus evolves.
2. Doesn't honor rubric-conditional blocks (`!*D` Defunctorum-only,
   `!*S` Solemn-only, `!*nD` not-Defunctorum, `!*SnD`, `!*RnD`).
3. Doesn't support per-cursus variants (Ordo67, OrdoN, OrdoOP, OrdoA,
   OrdoM, OrdoS).
4. The Last Gospel I just added is a literal copy — it should come
   from `Ordo.txt` lines 388–414 with proper substitution for the few
   days that override the Last Gospel (Palm Sunday, Christmas-day
   third Mass, etc.).

### Perl source
- `vendor/divinum-officium/web/www/missa/Latin/Ordo/Ordo.txt` (415 lines):
  the structured template with `&macros`, `&propername`-insertion
  points, and `!*RUBRIC` conditionals.
- `vendor/divinum-officium/web/www/missa/Latin/Ordo/Prayers.txt`
  (252 lines): named static prayer texts (`[Pater noster]`,
  `[Confiteor]`, `[IteMissa]`, `[Per Dominum]`, `[Gloria]`, etc.).
- `vendor/divinum-officium/web/cgi-bin/missa/ordo.pl::makemass()`:
  walks the template line by line. Resolves `&Macro` against
  Prayers.txt, replaces `&introitus`/`&collect`/`&lectio`/`&graduale`/
  `&evangelium`/`&offertorium`/`&secreta`/`&prefatio`/`&communicantes`/
  `&hancigitur`/`&Communio_Populi`/`&communio`/`&postcommunio`/
  `&itemissaest`/`&Ultimaev` with the day's computed propers,
  evaluates `!*RUBRIC` gates against the active mode.
- `vendor/divinum-officium/web/cgi-bin/missa/Cmissa.pl:52-53`:
  the Ordo-file picker per cursus.

### Refactor (Phase R1) — DONE
- ✅ `data/build_ordo_json.py`: reads `Ordo/Ordo*.txt` + `Prayers.txt`
  + `Prefationes.txt` from the submodule, emits `data/ordo_latin.json`.
  Output: 7 templates (Ordo, Ordo67, OrdoN, OrdoA, OrdoM, OrdoOP, OrdoS),
  44 prayers, 34 prefaces.
- ✅ `src/data_types.rs::OrdoLine` + `OrdoCorpus` shared between
  `build.rs` and the lib (postcard-encoded, brotli-compressed, ~50 KB
  shipped weight).
- ✅ `src/ordo.rs::render_mass(args)` walker. Mirrors
  `propers.pl::specials()`: applies `!*FLAG` flag-guards (D/R/S/nD/RnD/
  SnD), `!*&hookname` hook-guards (CheckQuiDixisti, CheckPax,
  CheckBlessing, CheckUltimaEv, placeattibi), and side-effect hooks
  (Introibo / GloriaM / Credo emitting `omit.` rubrics under the right
  conditions).
- ✅ `wasm.rs::compute_mass_full(year, month, day, rubric, solemn,
  rubrics) -> JSON` — exposes the rendered Ordinary as
  `ordinary: [...]` alongside the existing office / propers / rules.
  Auto-infers `defunctorum` from the winner's `officium` / `commune`
  metadata (covers All Souls + votive Cross via C9 commune).
  `compute_mass_json` removed — `compute_mass_full` is its strict
  superset.
- ✅ `demo/ordo.js` deleted; `demo/render.js` walks
  `mass.ordinary` from WASM. Solemn / show-rubrics checkboxes added
  to the form; Defunctorum is auto-detected by the engine.

**Outcome**: zero static Latin in JS; demo + Rust core share the
Ordo source-of-truth bundled from upstream. WASM bundle stayed
~1.3 MB (the new Ordo corpus added ~50 KB brotli; we pruned
`compute_mass_json` + `pull_rules` to compensate).

---

## B. Substring feast detection in `occurrence.rs::downgrade_post_1570_octave`

### Where
```rust
// src/occurrence.rs:563-584
let has_pre_1856_feast = officium.contains("Cordis Jesu")
    || officium.contains("Cordis Iesu")
    || officium.contains("Sacratissimi")
    || officium.contains("Patrocinii")
    || officium.contains("Patrocínii");
if is_pre_1856_demoter && has_pre_1856_feast { return 1.0; }

if is_pre_1925_demoter && officium.contains("Christi Regis") {
    return 1.0;
}
```

Plus `is_post_1570_octave_file()` in `mass.rs:1390+1409+1424` doing
similar string-match.

### Why it's wrong
- Brittle to rendering / orthography drift (`Iesu` vs `Jesu`).
- Doesn't generalize to other post-Tridentine feasts (Patrocinii BMV,
  Sacro Cuore di Maria, etc.).
- Mixes "what year was this feast introduced" with "what string does
  the corpus happen to render" — those should be separate.

### Perl source
The kalendar table chain handles this directly: a feast that was
*added* in 1856 doesn't appear in `Tabulae/Kalendaria/1570.txt`; it
appears for the first time in `1888.txt` or later. The Pius-V
kalendar (1570) just doesn't have a Sacred Heart entry, so under
`Rubric::Tridentine1570 → kalendar_layer = Pius1570`, the feast
naturally absents.

We're already using the layered kalendar table (`kalendaria_layers.rs`).
The hardcoded `downgrade_post_1570_octave` is a workaround for places
where the Sancti file ships a `[Rank]` body that the kalendar layer
doesn't suppress — but the kalendar lookup *should* be the source of
truth.

### Refactor (Phase R2)
Audit each Sancti file with a hardcoded post-1570 feast.
1. If the kalendar layer correctly suppresses it → trust the kalendar
   lookup, delete the substring-match.
2. If the kalendar layer claims to ship the feast but at a lower rank
   under T1570 → ensure `kalendaria_by_rubric.json` carries the right
   per-layer rank, drop the substring fallback.
3. Net: the kalendar table is the only source for "what feasts exist
   under what rubric".

`is_post_1570_octave_file` (mass.rs:1390+) probably has the same
shape; also needs auditing.

---

## C. Hardcoded date branches in occurrence

### Where
- `src/occurrence.rs:178-180`: `m == 1 && d == 12 && dow == 6` —
  the Jan-12-Sat anticipation patch I added last turn (closes 15
  fail-years).
- `src/occurrence.rs:616`: `month == 1 || (month == 2 && day == 1)`
  → `Commune/C10b` for Sat-BVM. Selects the BVM-after-Christmas
  commune by month.
- `src/transfer_table.rs:100`: `if month == 4 { 1 } else { 0 }` —
  leap-day shift.
- `src/date.rs:451-452`: `if leap_year(year) && month == 2 { if day
  == 24 { ... } }` — bissextile.
- `src/occurrence.rs:1087`: `if dow == 0 { ... }` — Sunday-special
  Tempora stem.

### Why partly right, partly wrong
- The leap-day arithmetic is a real Gregorian-calendar fact, not a
  hack — keep.
- The Jan-12-Sat anticipation is a *category* of "Sunday-Within-
  Octave-of-Epiphany when Octave Day = Sunday" — generalizable. Other
  known instances: Jan-7-Sun (when Epiphany is Sat Jan 6, suppresses
  Day II of Octave + the Sun-Within-Octave Mass). The Perl handles
  these via the `directorium`-driven Sunday-letter Transfer table at
  `Tabulae/Transfer/<letter>.txt` + season-specific rules in
  `horascommon.pl::precedence()`.
- The Sat-BVM month-based commune selection should derive from the
  resolved season (we have `Season` enum), not literal month numbers.

### Refactor (Phase R3)
- **C.1** Generalize Sunday-Within-Octave anticipation: define a
  table of "(temporal-week, dow) → anticipated stem" pairs derived
  from upstream Sunday-letter behaviour. Replace the Jan-12 hardcode
  with a single lookup. Also closes the analogous Quad6-2 / Pasc1-0t
  patterns in the long tail.
- **C.2** Sat-BVM commune lookup: switch from `month == 1 ||
  (month == 2 && day == 1)` to `match office.season { ... }`. The
  decision tree is already documented in the Perl
  `horascommon.pl:401-420` against the resolved week label.
- **C.3** Leave the bissextile and dow==0 date-arithmetic alone —
  those are calendar primitives.

---

## D. Inline body-conditional grammar in `mass.rs::apply_body_conditionals_1570`

### Where
`src/mass.rs:466-540` and surrounding. Has hardcoded:
- Stopword list: `sed`, `vero`, `atque`, `attamen`, `deinde`.
- Scope-keyword list: `omittitur`, `omittuntur`, `semper`, `loco`,
  `versus`, `versuum`, `hæc`, `hac`, `haec`.
- Predicate-subject list: `rubrica`, `rubricis`, `communi`.
- Hardcoded named predicates: `tridentina`, `monastica`, `innovata`,
  `summorum pontificum`.

These are spread across `apply_body_conditionals_1570`,
`eval_simple_conditional_1570`, `eval_alt_1570`, and
`rubrica_predicate_matches`.

### Why it's a hack
Perl `SetupString.pl::parse_conditional` + `process_conditional_lines`
drives the same grammar from a small `%stopword_weights` and
`%scope_keywords` table. Our distributed approach makes it hard to
add a new predicate (every named subject needs new code in 4 places).

### Perl source
`vendor/divinum-officium/web/cgi-bin/DivinumOfficium/SetupString.pm`
ll. ~80–170 — the data tables. The state machine on top is ~110 lines.

### Refactor (Phase R4)
Move the tables to `src/data/conditionals.rs` (or load from a tiny
`data/conditionals.toml`). The state machine in `apply_body_conditionals`
becomes the literal port of `process_conditional_lines`. Per the
existing audit at `RUST_PERL_UNIFICATION_AUDIT.md` H1, this is the
right time to do it.

---

## E. Substring `RankKind` dispatch in `precedence.rs::rank_kind_from_label`

### Where
`src/precedence.rs:143-166`:
```rust
} else if l.contains("feria") || l.contains("vigilia")
       || l.contains("sabbato") {
    RankKind::Feria
} else if l.contains("in octava") || l.contains("die octava") {
    RankKind::OctaveDay
}
```

### Why it's a hack
Brittle to label-string drift. The corpus already carries explicit
rank metadata (`rank_class`, `rank_num`); the rank *kind* should
follow from those numerics or from a side table, not from substring
matching the human-readable label.

### Refactor (Phase R5)
Build a `RankKind` derivation rule from `(rank_num, kalendar layer
metadata)` via a small lookup. The numeric `rank_num` already
distinguishes Class I/II/III/IV/Vigil/Octave-Day. The substring
match is a stale heuristic from before the kalendar table was
threaded through.

---

## Phased plan + sequencing

| Phase | What                                                    | Touches            | Risk |
|-------|---------------------------------------------------------|--------------------|------|
|  R1   | Ordo.txt + Prayers.txt → bundled corpus + Rust renderer + WASM `compute_mass_full`. Demo loses static-content hardcode. | data/, src/, demo/ | Low. New code path; existing `compute_mass_json` stays for back-compat. |
|  R2   | Replace `officium.contains("Christi Regis")` etc. with kalendar-layer lookup. | occurrence.rs, mass.rs | Medium. Need to verify each known-failing case still passes. |
|  R3   | Generalize Sunday-Within-Octave anticipation + Sat-BVM season-based commune. | occurrence.rs | Medium. Removes the Jan-12 hardcode; closes more long-tail patterns at once. |
|  R4   | Conditional-grammar tables + faithful `process_conditional_lines` port. | mass.rs (split → conditional.rs) | Medium-high. Already scoped in `RUST_PERL_UNIFICATION_AUDIT.md` H1. |
|  R5   | `RankKind` derivation from numerics + kalendar metadata; drop substring matching in precedence. | precedence.rs | Low. Mostly mechanical. |

Each phase should land independently, with a 1-year regression smoke
test after to confirm no per-day output drift before moving to the
next. The full ±50 year sweep runs only at the end of each phase.

## Out-of-scope for this audit

- The `regression.rs` Perl-rubric-stripper substring lists (these
  legitimately track Perl-side rendering markers, not corpus data).
- Test-only `assert_eq` literals in `#[cfg(test)]` blocks.
- Bissextile-day arithmetic and DOW math in `date.rs` — those are
  calendar primitives, not corpus lookups.
