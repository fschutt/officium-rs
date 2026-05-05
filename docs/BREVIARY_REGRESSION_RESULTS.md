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
