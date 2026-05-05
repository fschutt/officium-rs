# Breviary regression — running tally

Tracks the Office-side year-sweep against upstream Perl. Mirrors
`REGRESSION_RESULTS.md` for the Mass side.

## Sweep cadence

```
cargo run --release --bin office_sweep -- \
    --year 2026 --hour Vespera --rubric 'Tridentine - 1570' \
    --section Oratio
```

Each cell:

1. Derive day-key via `precedence::compute_office` (Mass-side
   calendar-resolution).
2. Auto-derive `next_day_key` via the same path; for `Vespera`,
   `horas::first_vespers_day_key` swaps to tomorrow's office when
   tomorrow outranks today (first vespers concurrence).
3. Walk the Ordinarium template + per-day chain via
   `horas::compute_office_hour`.
4. Extract the named section's body via
   `regression::rust_office_section`.
5. Shell to `scripts/do_render.sh` for the upstream HTML; extract
   the same section via `regression::extract_perl_sections`.
6. Compare via `regression::compare_section_named` (normalised
   substring containment).

## Progress (Vespera × Tridentine 1570 × Oratio)

Baselines on a 30-day January slice:

| Slice | Pass rate | Match | Differ | RustBlank | Notes |
|------:|----------:|------:|-------:|----------:|-------|
| 1 (initial)  | 26.67% | 8/30  | 11     | 11        | First measurement after slice 2 wired the loop |
| 3            | 36.67% | 11    | 13     | 6         | `parse_vide_targets` accepts `ex Sancti/MM-DD` (Octave inherit) |
| 4            | 46.67% | 14    | 10     | 6         | Auto-derive `next_day_key` for first-vespers + `vide Sancti/...` chain |
| 5            | 50.00% | 15    | 9      | 6         | `expand_at_redirect` whole-body `@Path` resolver |
| 6            | 56.67% | 17    | 9      | 4         | Strip trailing `;`/`,` from path tokens (`vide Sancti/12-27;`) |
| 7            | 60.00% | 18    | 12     | **0**     | Hyphenated commune subkeys (`vide C6-1`) + Tempora-feria→Sunday fallback |
| 8            | 63.33% | 19    | 11     | 0         | `N.` saint-name placeholder substitution from per-day `[Name]` |

**Cumulative gain across slices 3-8: 26.67% → 63.33% (+37 pts)**.

60-day slice (Jan + Feb): **40/60 = 66.67%** — February's wider
Sancti coverage matches more cleanly than January's heavy
Octave indirection.

## Slice 10: per-hour distribution + Matutinum rubric strip

Added `--hour all` mode to `office_sweep` that walks all 8
canonical hours per date and reports match-rate per hour.

**14-day × 8-hour Oratio sweep, T1570:**

| Hour          | Pass rate  | Notes |
|---------------|-----------:|-------|
| Matutinum     | 13/14 (92.86%) | was 0% — fixed slice 10 |
| Laudes        | 13/14 (92.86%) |  |
| Prima         | 0/14   (0.00%) | fixed `$oratio_Domine` not expanded |
| Tertia        | 13/14 (92.86%) |  |
| Sexta         | 13/14 (92.86%) |  |
| Nona          | 13/14 (92.86%) |  |
| Vespera       | 13/14 (92.86%) |  |
| Completorium  | 0/14   (0.00%) | fixed `$oratio_Visita` not expanded |
| **Aggregate** | **78/112 (69.64%)** | up from 58.04% pre-slice-10 |

Slice-10 fix: `rust_office_section` now strips Ordinarium-
template rubric directives — `(sed rubrica X)`,
`(rubrica X dicitur)`, `$rubrica <Name>` — from extracted
section bodies. These are template-level conditionals tied to
non-active rubrics (Cisterciensis, Monastic, Triduum) that
the walker emits but the Perl render skips when the gate
doesn't fire. Matutinum was the worst offender: its `#Oratio`
template ends with three such lines after the actual Oratio
body, which forced a false Differ on every cell.

Prima and Completorium remain at 0% because their `#Oratio`
template embeds a FIXED Oratio (`$oratio_Domine` for Prima,
`$oratio_Visita` for Completorium) plus surrounding macros
(Pater noster, Kyrie, Dominus vobiscum, Per Dominum). The
walker emits `$oratio_<Name>` as a literal token; Perl looks
the macro up in `Psalterium/Common/Prayers` and renders the
expanded prayer. Slice 11 fixes this.

## Remaining divergence patterns (slice 8 baseline)

The 11 residual Differs on the 30-day Jan slice all fall into
**Tempora-vs-Sancti rank precedence** — same gap already
documented on the Mass side (`REGRESSION_RESULTS.md` Phase
7+ tasks). For the day's compute_office winner, Rust picks a
Sancti file where Perl picks a Tempora ferial (or vice versa),
so the two sides emit different proper bodies.

Closing this on the Mass side automatically closes it on the
Office side — both consume `precedence::compute_office`.

Specific cases observed:
- 01-14-2026 (St. Hilary) — Perl picks Suffrage of Peace
  (Tempora ferial) for the Vespera oratio.
- 01-23-2026 (St. Emerentiana, Friday) — Perl picks first
  Vespers of Saturday-BVM (with Timothy commemoratio).

## Patterns *closed* during B8

The B8 chain-resolution work landed several breviary-specific
fixes that wouldn't have surfaced on the Mass side:

1. `parse_vide_targets` now handles **all** chain shapes the
   upstream rule body uses:
   - `vide CXX[a]` / `vide CXX[a]-N[a]` (Commune + sub-key)
   - `ex Sancti/MM-DD` / `ex Tempora/Foo` (Octave inherit)
   - `@Sancti/MM-DD` / `@Tempora/Foo` (parent-inherit)
   - `vide Sancti/MM-DD;` / `vide Tempora/Foo;` (with trailing `;`)
2. `commune_chain` falls through to `Tempora/<season>-0` for
   ferial/octave-tail keys (`Tempora/Epi3-4` → `Tempora/Epi3-0`).
3. `expand_at_redirect` resolves whole-body `@Path` and
   `@Path:Section` redirects (`Sancti/01-05 [Oratio] = @Tempora/Nat1-0`).
4. `substitute_saint_name` interpolates the per-day file's
   `[Name]` field into Commune `N.` placeholders (`beáti N. → beáti
   Pauli`).
5. `first_vespers_day_key` (called by `office_sweep`) swaps
   today's Vespera key to tomorrow's office when tomorrow
   outranks today.

## Patterns *attempted and reverted*

- **Mass-side `expand_macros` on Office bodies** (slice 9
  attempt): expanding `$Per Dominum`/`$Per eumdem` macros via
  `crate::mass::expand_macros` regressed pass-rate from 63.33%
  to 46.67%. The Mass-side prayer expansion text doesn't align
  with what Perl renders for Office bodies — the comparator's
  substring-match was already accepting the unexpanded form
  (Rust's body up to the macro marker matched the prefix of
  Perl's expanded text). Reverted; left as a known divergence.
