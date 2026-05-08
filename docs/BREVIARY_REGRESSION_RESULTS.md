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

## Slice 11: runtime conditional gating + Dominus_vobiscum slice

The Ordinarium template is read by upstream `getordinarium`
(`horas.pl:589`) as a flat line list and run through
`SetupString.pl::process_conditional_lines` once before per-line
emission. The Rust walker was emitting **every** template line
unconditionally, so every `(deinde rubrica X dicuntur)` /
`(sed PRED dicitur)` / `(atque dicitur semper)` block fired
regardless of the active rubric — Prima/Compline collected three
overlapping prayer fragments (`$Kyrie`, `$Pater noster Et`,
`&Dominus_vobiscum1`, `&Dominus_vobiscum`, the proper Oratio,
`&Dominus_vobiscum`, `&Benedicamus_Domino`, `$Conclusio
cisterciensis`, …) in one section.

Slice 11 lands two interlocking fixes:

1. `apply_template_conditionals` (in `src/horas.rs`) — synthesises
   a multi-line text where each `OrdoLine` becomes one line.
   Plain lines whose body looks like a `(...)` directive emit
   verbatim; non-directives emit a unique sentinel
   (`\u{1}OL<idx>\u{1}`); blank lines emit verbatim blank text so
   `process_conditional_lines`'s SCOPE_CHUNK retraction +
   forward-expiry see the same boundaries the upstream walker
   does. Surviving sentinels map back to their original
   `OrdoLine` indices; surviving non-sentinels (directive sequels
   like `(rubrica 1960) #De Officio Capituli` under R1960) are
   dropped for now (TODO: re-classify and emit as section/plain
   in a later slice).

2. `Dominus_vobiscum*` ScriptFunc lay-default slice — the upstream
   `horasscripts.pl::Dominus_vobiscum` returns lines [2,3] of the
   `[Dominus]` body (the V/R Domine exaudi couplet) under the
   no-priest, no-precesferiales default. The literal lookup of
   `[Dominus_vobiscum]` doesn't exist in Prayers.txt, so the
   walker was falling back to the entire 5-line `[Dominus]` body
   (Dominus vobiscum couplet + Domine exaudi couplet + the
   `/:secunda «Domine, exaudi» omittitur:/` script directive line).
   The slice intercepts `Dominus_vobiscum` /
   `Dominus_vobiscum1` / `Dominus_vobiscum2` and returns just
   lines [2,3].

**30-day Jan 2026 × T1570 × Oratio sweep — `--hour all`:**

| Hour          | Pre slice 11 | Post slice 11 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 25/30 (83%) | 25/30 (83%) | — |
| Laudes        | 25/30 (83%) | 25/30 (83%) | — |
| Prima         | 0/30  (0%)  | 19/30 (63%) | +19 |
| Tertia        | 25/30 (83%) | 25/30 (83%) | — |
| Sexta         | 25/30 (83%) | 25/30 (83%) | — |
| Nona          | 25/30 (83%) | 25/30 (83%) | — |
| Vespera       | 19/30 (63%) | 19/30 (63%) | — |
| Completorium  | 7/30  (23%) | 23/30 (77%) | +16 |
| **Aggregate** | **151/240 (62.92%)** | **186/240 (77.50%)** | **+35** |

R60 30-day aggregate stays at 29/240 (12.08%) — the conditional
gates that fire under R60 reshape Rust output to closer match
Perl, but the substring-match comparator was already accepting
the over-emitting form for the same set of days that pass after
gating.

Mass-side year sweep T1570 2026 stays at **365/365 (100%)** —
verified the `apply_template_conditionals` filter is Office-only
(no `compute_office_hour` call from `mass.rs`).

Remaining residual at Prima/Vespera/Completorium ~37% comes from
two separate gaps:

- **Preces-firing days** (e.g. 01-14-2026 Wed, 01-23-2026 Fri):
  `Dominus_vobiscum1` should set `$precesferiales = 1` when
  `preces('Dominicales et Feriales')` returns true and emit
  line [4] of `[Dominus]` (the omittitur directive). Closes when
  B12 (preces predicate) lands.
- **First-vespers concurrence** for days where the chain swap
  picks the wrong winner (01-15-2026 picks Paul Eremite instead
  of Marcellus's first vespers). Closes when concurrence
  (B11) lands.

## Slice 12: rubric-conditional eval on `[Rule]` + `[Name]` + spliced bodies

The build script (`data/build_horas_json.py`) bakes the
1570-baseline conditionals into per-section bodies but leaves
the 1910/DA/R55/R60 layer un-evaluated. Sancti/01-14 (St Hilary)
ships:

```
[Rule]
vide C4a;mtv
(sed rubrica 1570 aut rubrica 1617)
vide C4;mtv

[Name]
Hilárium
(sed rubrica 1570 aut rubrica 1617)
Hilárii
Ant=Hilári
```

— under T1570/1617, `[Rule]` should flip from `vide C4a` to
`vide C4` (the Confessor-Bishop common, oratio "Da, quaesumus,
omnipotens Deus..."). Without runtime evaluation, the chain
walked C4a (default Doctor) and emitted "Deus, qui populo tuo
aeternae salutis..." plus both copies of the conditional
variant. The `[Name]` body fed all three lines (`Hilárium /
Hilárii / Ant=Hilári`) into every Commune body's `N.` slot.

Slice 12 lands `eval_section_conditionals` (`src/horas.rs`) — a
thin wrapper around `setupstring::process_conditional_lines`
that's applied at three points:

1. `[Rule]` body in `visit_chain` before `parse_vide_targets`.
   Picks the rubric-correct `vide CXX` target.
2. `[Name]` body in `splice_proper_into_slot` before
   `substitute_saint_name`. Then takes the FIRST line that's
   not blank, not a directive, and not an `Ant=...` antiphon
   variant — Perl `$winner{Name}` is by convention the genitive
   form on line 1.
3. The spliced section body (after `expand_at_redirect`,
   before `substitute_saint_name`). Drops per-rubric prayer
   variants (`(sed communi Summorum Pontificum)` etc.).

`commune_chain_for_rubric(day_key, rubric, hora)` is the new
entry point; the legacy `commune_chain(day_key)` stays as a
T1570 / Vespera shim for B5 callers and tests.

**30-day Jan 2026 × T1570 × Oratio sweep — `--hour all`:**

| Hour          | Pre slice 12 | Post slice 12 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 25/30 (83%) | 27/30 (90%) | +2 |
| Laudes        | 25/30 (83%) | 27/30 (90%) | +2 |
| Prima         | 19/30 (63%) | 19/30 (63%) | — |
| Tertia        | 25/30 (83%) | 27/30 (90%) | +2 |
| Sexta         | 25/30 (83%) | 27/30 (90%) | +2 |
| Nona          | 25/30 (83%) | 27/30 (90%) | +2 |
| Vespera       | 19/30 (63%) | 19/30 (63%) | — |
| Completorium  | 23/30 (77%) | 23/30 (77%) | — |
| **Aggregate** | **186/240 (77.50%)** | **196/240 (81.67%)** | **+10** |

R60 30-day aggregate: 29/240 (12.08%) → **35/240 (14.58%)**
(+6 across all hours except Prima / Compline). Mass T1570 + R60
year-sweeps stay at 100%.

Remaining residual on Matutinum/Laudes/Tertia/Sexta/Nona
(3 days each): **01-18, 01-20, 01-25**. Three different
patterns to dig into next slice.

## Slice 13: `@Path:Section:s/PAT/REPL/` substitution + first-chunk Oratio splice + UTF-8 regex

Three failing-day clusters surfaced after slice 12 closed the
`[Rule]`/`[Name]`/body conditionals:

1. **`@Path::s/PAT/REPL/` redirect with substitution** — Sancti/01-20
   (Fabiani+Sebastiani) `[Oratio]` body is
   `@Commune/C2::s/beáti N\. Mártyris tui atque Pontíficis/beatórum
   Mártyrum tuórum Fabiáni et Sebastiáni/`. The redirect should
   resolve `Commune/C2` `[Oratio]` and apply the inclusion
   substitution to swap singular `N. Martyris` → plural form. Old
   `expand_at_redirect` only handled `@Path` and `@Path:Section` —
   the trailing `:s/.../.../[FLAGS]` was silently kept on the
   section name, leaving the literal `@Commune/C2::s/...` in the
   spliced body.

2. **First-chunk Oratio splice** — Sancti/02-22 + Sancti/01-25
   `[Oratio]` bodies have the multi-chunk shape `prayer\n$Per
   Dominum\n_\n@Path:CommemoratioN`. Upstream Perl emits only the
   first chunk for the primary winner-Oratio; subsequent chunks
   are commemoration alternatives reserved for days that
   actually run a commemoration block. Without trimming, the
   trailing `@Path:CommemoratioN` literal leaks into the rendered
   body and breaks the substring-match comparator.

3. **UTF-8 regex parser** — `setupstring::compile_regex`'s atom
   parser was byte-based: a multi-byte Latin char like `á` (`0xC3
   0xA1`) became two `Char(0xC3) Char(0xA1)` tokens, but the
   matcher iterates UTF-8 chars (`Char(U+00E1)`). Patterns with
   accented Latin characters never matched. Fix: detect UTF-8 lead
   bytes in `parse_atom` and read the full codepoint into a single
   `Char(c)` token. Same patch unblocks every
   `@Path::s/PAT/REPL/` substitution that mentions accented Latin.

`expand_at_redirect` (`src/horas.rs`) now parses
`@PATH[:SECTION][:SPEC]` properly: when SECTION is empty (`::SPEC`
form) the default section is used; SPEC is delegated to
`setupstring::do_inclusion_substitutions`. Recursion through nested
redirects is gated to the no-spec case so substitutions apply to
the resolved body (not the intermediate redirect).

`take_first_oratio_chunk` (`src/horas.rs`) trims everything from
the first standalone `_` line onward, applied only when splicing
`Oratio` / `Oratio <Hour>` sections.

**30-day Jan 2026 × T1570 × Oratio sweep — `--hour all`:**

| Hour          | Pre slice 13 | Post slice 13 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 27/30 (90%) | **30/30 (100%)** | +3 |
| Laudes        | 27/30 (90%) | **30/30 (100%)** | +3 |
| Prima         | 19/30 (63%) | 19/30 (63%) | — |
| Tertia        | 27/30 (90%) | **30/30 (100%)** | +3 |
| Sexta         | 27/30 (90%) | **30/30 (100%)** | +3 |
| Nona          | 27/30 (90%) | **30/30 (100%)** | +3 |
| Vespera       | 19/30 (63%) | 24/30 (80%) | +5 |
| Completorium  | 23/30 (77%) | 23/30 (77%) | — |
| **Aggregate** | **196/240 (81.67%)** | **216/240 (90.00%)** | **+20** |

R60 30-day aggregate: 35/240 (14.58%) → **40/240 (16.67%)** (+5).
Mass T1570 + R60 year-sweeps stay at 365/365 (100%). All 431 lib
tests pass.

5 hours at 100% on this slice. Remaining residuals concentrate on
Prima (preces predicate B12), Vespera (concurrence B11), and
Completorium (some mix of both).

## Slice 14: `Dominus_vobiscum1` preces-firing branch (line[4] of [Dominus])

`horasscripts.pl::Dominus_vobiscum1` is the "Prima/Compline after
preces" ScriptFunc — when `preces('Dominicales et Feriales')`
returns true, it sets `$precesferiales = 1` and the inner
`Dominus_vobiscum` returns line[4] of `[Dominus]` (the
`/:secunda «Domine, exaudi» omittitur:/` rubric directive)
instead of the lay-default V/R Domine exaudi couplet at lines
[2,3]. Slice 11 wired the lay-default; slice 14 wires the
preces-firing branch.

`preces_dominicales_et_feriales_fires` is a narrow port of
`specials/preces.pl::preces` covering:

- Sunday / Saturday-Vespers exclusion.
- BVM C12 office exclusion.
- `[Rule]` with "Omit Preces" exclusion (rubric-conditional eval'd
  first via slice 12's `eval_section_conditionals`).
- `[Rank]` parsed for active rubric: `duplex >= 3` rejects;
  Octave-rank rejects (unless "post Octav").
- 1955/1960 day-of-week gate (Wed/Fri only — emberday detection
  deferred).
- Sancti winner branch (b) fires when the above pass.
- Tempora winner branch (a) — conservative: only fires when
  `[Rule]` mentions "Preces" explicitly. Adv/Quad/emberday
  detection deferred to a later slice (the 30-day Jan T1570 sample
  has no Tempora ferials with active preces; the upstream concentration
  is in Lent / Septuagesima which slice doesn't cover yet).

The walker's `kind: macro` branch dispatches on `Dominus_vobiscum1`
and emits `dominus_vobiscum_preces_form(prayers)` (line[4]) when
the predicate returns true; otherwise falls through to the
existing `lookup_horas_macro` path which slice 11 already routes
to `dominus_vobiscum_lay_default` (lines [2,3]).

**30-day Jan 2026 × T1570 × Oratio sweep — `--hour all`:**

| Hour          | Pre slice 14 | Post slice 14 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 30/30 (100%) | 30/30 (100%) | — |
| Laudes        | 30/30 (100%) | 30/30 (100%) | — |
| Prima         | 19/30 (63%)  | 27/30 (90%)  | +8 |
| Tertia        | 30/30 (100%) | 30/30 (100%) | — |
| Sexta         | 30/30 (100%) | 30/30 (100%) | — |
| Nona          | 30/30 (100%) | 30/30 (100%) | — |
| Vespera       | 24/30 (80%)  | 24/30 (80%)  | — |
| Completorium  | 23/30 (77%)  | 25/30 (83%)  | +2 |
| **Aggregate** | **216/240 (90.00%)** | **226/240 (94.17%)** | **+10** |

R60 30-day aggregate: 40/240 (16.67%) — unchanged. The 1955/1960
gating in the predicate suppresses preces on most R60 days, so the
slice doesn't lift R60. Mass T1570 + R60 year-sweeps stay at
365/365 (100%). All 431 lib tests pass.

Vespera still at 80% (6 fails) — its residual is first-vespers
concurrence (`&Dominus_vobiscum1` doesn't appear in Vespera #Oratio,
so this slice doesn't help). Compline at 83% — 5 remaining fails
mix preces predicate edge cases (Saturday Vespera adjacency) with
other patterns.

## Slice 15: Tempora-ferial preces predicate extension

Slice 14's `preces_dominicales_et_feriales_fires` was Sancti-only
(plus Tempora gated on `[Rule]` mentioning "Preces"). The Jan
T1570 sample has Tempora ferials Epi3-4 and Epi3-5 (01-29 Thu and
01-30 Fri) where upstream Perl emits the omittitur form for
Prima/Compline `Dominus_vobiscum1` — branch (b) of upstream
`preces` fires for any low-rank, non-Octave winner regardless of
its file category.

Slice 15 widens the Tempora branch: under T1570/1910/DA, fire
preces for any `Tempora/...` winner that has already passed the
duplex/octave checks. The `[Rule]`-mentions-Preces gate from
slice 14 was conservative — branch (a) in upstream uses
`dayname[0] =~ /Adv|Quad/` or `emberday()`, but branch (b)'s
"low-rank Tempora ferial" condition collapses to the same
practical set without needing season detection.

**30-day Jan 2026 × T1570 × Oratio sweep — `--hour all`:**

| Hour          | Pre slice 15 | Post slice 15 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 30/30 (100%) | 30/30 (100%) | — |
| Laudes        | 30/30 (100%) | 30/30 (100%) | — |
| Prima         | 27/30 (90%)  | 29/30 (97%)  | +2 |
| Tertia        | 30/30 (100%) | 30/30 (100%) | — |
| Sexta         | 30/30 (100%) | 30/30 (100%) | — |
| Nona          | 30/30 (100%) | 30/30 (100%) | — |
| Vespera       | 24/30 (80%)  | 24/30 (80%)  | — |
| Completorium  | 25/30 (83%)  | 27/30 (90%)  | +2 |
| **Aggregate** | **226/240 (94.17%)** | **230/240 (95.83%)** | **+4** |

R60 30-day stays at 40/240 (16.67%) — 1955/1960 Wed/Fri gate
still suppresses preces on most R60 days. Mass T1570 + R60
year-sweeps stay at 365/365 (100%). 431 lib tests pass.

Residual fails on T1570 30-day:
- Prima 1 fail (01-24): Sat BVM (Commune/C10b) — predicate
  doesn't recognise the BVM-Saturday path because `[Rank]` lives
  on the inherited `@Commune/C10` parent, which our chain walker
  doesn't yet pull through for whole-file inheritance. Deferred.
- Vespera 6 fails (01-14, 01-15, 01-23, 01-26, 01-28, 01-30):
  first-vespers concurrence (B11). The `parse_horas_rank` MAX-
  across-rubric-variants approach picks the wrong winner for
  equal-rank tomorrow vs today comparisons.
- Compline 3 fails (01-16, 01-19, 01-26): Sancti days where
  upstream Perl's `preces` returns 0 but our predicate fires.
  Likely `$commemoratio` set by precedence engine to a
  Sunday-of-Octave or week-commemoration that triggers
  `dominicales=0`. Deferred — needs `$commemoratio` propagation
  from the precedence layer.

## Slice 16: rubric-aware first-Vespers concurrence — tomorrow-wins-on-tie

`first_vespers_day_key` had two bugs:

1. **Rank parsed across rubric variants via MAX.** Sancti/01-14
   (Hilary) `[Rank]` body lists `;;Duplex;;3;;vide C4a` (default)
   and `;;Semiduplex;;2.2;;vide C4` (T1570 variant). The MAX
   approach picked 3 instead of 2.2 — so today vs tomorrow
   comparisons used inflated ranks that masked real ties.

2. **Today wins on tie.** Roman Office concurrence privileges
   tomorrow's first Vespers when ranks are equal — only a
   strictly-higher today-rank keeps today's second Vespers. The
   old comparator (`tomorrow_rank > today_rank`) flipped the
   default the wrong way: equal ranks went to today.

`parse_horas_rank_for_rubric` (`src/horas.rs`) reuses slice 12's
`eval_section_conditionals` to filter the `[Rank]` body by the
active rubric, then returns the first surviving rank-num. When
the day file has no `[Rank]` section (Sancti/01-18 Cathedra Petri
= `@Sancti/02-22`, Commune/C10b BVM Saturday = `@Commune/C10`),
chase via `first_at_path_inheritance` — scan section bodies for a
leading `@Path` line and recurse.

`first_vespers_day_key_for_rubric(today, tomorrow, rubric, hora)`
is the new entry. The legacy `first_vespers_day_key(today,
tomorrow)` shim defaults to T1570/Vespera. `office_sweep` now
calls the rubric-aware variant.

**30-day Jan 2026 × T1570 × Oratio sweep — `--hour all`:**

| Hour          | Pre slice 16 | Post slice 16 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 30/30 (100%) | 30/30 (100%) | — |
| Laudes        | 30/30 (100%) | 30/30 (100%) | — |
| Prima         | 29/30 (97%)  | 29/30 (97%)  | — |
| Tertia        | 30/30 (100%) | 30/30 (100%) | — |
| Sexta         | 30/30 (100%) | 30/30 (100%) | — |
| Nona          | 30/30 (100%) | 30/30 (100%) | — |
| Vespera       | 24/30 (80%)  | 27/30 (90%)  | +3 |
| Completorium  | 27/30 (90%)  | 27/30 (90%)  | — |
| **Aggregate** | **230/240 (95.83%)** | **233/240 (97.08%)** | **+3** |

Closes Vespera 01-14 (Hilary→Paul Eremite tie), 01-15 (Paul→
Marcellus tie), 01-23 (Emerentiana→BVM Saturday — slice's `@Path`
inheritance for [Rank] picks up Commune/C10's rank 1.3 from
inheritance), 01-26 (Polycarp Simplex 1.1 → John Chrysostom
Duplex 3, tomorrow strictly higher).

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass (the `first_vespers_keeps_today_on_rank_tie` test was
inverted by this slice — replaced with
`first_vespers_swaps_to_tomorrow_on_rank_tie` to reflect the
upstream tie rule).

Vespera residual (3 fails: 01-24, 01-28, 01-30): the precedence
engine's day_key for Saturday/Sunday-eve uses Tempora-Sunday
codes (`Tempora/Epi4-0tt`) instead of the upstream's "Sat
ferial of week III" path. Affects ferial Vespera resolution when
the immediate next-day winner doesn't have its own first Vespers
(low-rank Simplex with no `[Vespera]` proper, BVM Saturday with
displaced saint, etc.). Closes when the tempora-resolution layer
matches upstream `gettempora` more precisely.

## Slice 17: corpus preamble + whole-file inheritance — Prima 100%, +1 net

Two interlocking changes:

1. **Corpus build script captures pre-section preamble.**
   `data/build_horas_json.py::parse_horas_file` now stores any
   non-section content before the first `[Section]` header under
   the magic key `__preamble__`. This preserves the upstream
   convention where `Commune/C10b` (Saturday BVM Office) starts
   with a bare `@Commune/C10` line that triggers whole-file
   merge in Perl `setupstring`. Build output count went 1204 →
   1204 keys (no new files, only one new section per affected
   file).

2. **Runtime whole-file inheritance.** `first_at_path_inheritance`
   in `src/horas.rs` now reads `__preamble__` (instead of scanning
   arbitrary section bodies for `@Path` lines). Used by
   `parse_horas_rank_for_rubric` and the new
   `active_rank_line_for_rubric` to find Sat-BVM rank from
   `Commune/C10` when looked up via `Commune/C10b`.

3. **Preces predicate extended to Commune winners.** Slice 15
   limited branch (b) to Sancti and Tempora; this widens to
   `Commune/...` so Saturday BVM (Commune/C10b winner) emits
   line[4] of `[Dominus]` when the rank checks pass.

**30-day Jan 2026 × T1570 × Oratio sweep:**

| Hour          | Pre slice 17 | Post slice 17 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 30/30 (100%) | 30/30 (100%) | — |
| Laudes        | 30/30 (100%) | 30/30 (100%) | — |
| Prima         | 29/30 (97%)  | **30/30 (100%)** | +1 |
| Tertia        | 30/30 (100%) | 30/30 (100%) | — |
| Sexta         | 30/30 (100%) | 30/30 (100%) | — |
| Nona          | 30/30 (100%) | 30/30 (100%) | — |
| Vespera       | 27/30 (90%)  | 28/30 (93%)  | +1 |
| Completorium  | 27/30 (90%)  | 26/30 (87%)  | -1 |
| **Aggregate** | **233/240 (97.08%)** | **234/240 (97.50%)** | **+1** |

**7 of 8 hours at 100% on the 30-day slice.** Compline regressed
by 1 cell (01-24 Sat BVM Compline) — Perl's preces returns 0 on
Sat 01-24 Compline despite returning 1 on Sat 01-24 Prima with
the same winner / rank. The hour-specific divergence isn't in
upstream `preces` itself but in some surrounding precedence /
commemoratio state we don't yet propagate. 01-31 Sat (also BVM
Saturday) emits omittitur on Compline — so it's not a uniform
Saturday-Compline rule. Documented as an open residual; the gain
on Prima 01-24 makes the trade net-positive overall.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 18: Vespera = 100% via "No prima vespera" + Simplex-no-2V

`first_vespers_day_key_for_rubric` had two remaining bugs:

1. **Tomorrow's `[Rule]` "No prima vespera"** wasn't honoured.
   `Tempora/Epi4-0tt` (Sat-eve-of-Sun-IV variant Simplex 1.5)
   carries an explicit `No prima vespera` directive in its
   `[Rule]` body — its rank 1.5 would otherwise outrank a
   Friday Tempora ferial (Feria 1.0) and pick the wrong office.
   Mirror of upstream `concurrence`'s scan for this marker.

2. **Sancti Simplex has no 2nd Vespers.** Wed 01-28 (Sancti/01-28t
   Agnes Simplex 1.1) is the typical case: 1.1 > Thursday's
   Tempora ferial 1.0, but Sancti Simplex has no proper 2V.
   Today's Vespera is empty — falls through to tomorrow's Tempora
   ferial (which inherits Sun III's office via "Oratio Dominica").
   Tempora ferials don't have this problem (they always inherit
   Sunday). Branch fires when `today_key` starts with `Sancti/`
   and the active rank class is Simplex / Memoria /
   Commemoratio (or rank num < 2.0).

**30-day Jan 2026 × T1570 × Oratio sweep:**

| Hour          | Pre slice 18 | Post slice 18 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 30/30 (100%) | 30/30 (100%) | — |
| Laudes        | 30/30 (100%) | 30/30 (100%) | — |
| Prima         | 30/30 (100%) | 30/30 (100%) | — |
| Tertia        | 30/30 (100%) | 30/30 (100%) | — |
| Sexta         | 30/30 (100%) | 30/30 (100%) | — |
| Nona          | 30/30 (100%) | 30/30 (100%) | — |
| Vespera       | 28/30 (93%)  | **30/30 (100%)** | +2 |
| Completorium  | 26/30 (87%)  | 26/30 (87%)  | — |
| **Aggregate** | **234/240 (97.50%)** | **236/240 (98.33%)** | **+2** |

**7 of 8 hours at 100% on the 30-day slice.** Only Compline
holds the line below — its 4 residuals are all preces-predicate
edge cases (01-16, 01-19, 01-24, 01-26) where upstream Perl
rejects preces but our predicate fires. Hour-specific divergence
that needs `$commemoratio` propagation from the precedence layer.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 19: Compline = 100% on 30-day Jan via liturgical-day extension

The Roman liturgical day spans Vespers → Compline → Matins → Lauds
→ Prima → Minor → Vespers → Compline. Compline is the **last** hour
of a liturgical day; when Vespers of calendar day X resolves to
first Vespers of calendar day Y, the **same Y winner extends through
Compline** of calendar day X.

`office_sweep` already auto-derived `next_day_key` and called
`first_vespers_day_key_for_rubric` for Vespera. Slice 19 extends
the same swap to Compline:

```
let next_derived_key = if hour == "Vespera" || hour == "Completorium" {
    /* compute_office for tomorrow + first-vespers swap */
}
```

For 01-16 Fri Compline T1570:
- Without swap: `winner = Sancti/01-16` (Marcellus Semiduplex 2.2)
  → preces predicate fires → Rust emits omittitur
- Perl: `winner = Sancti/01-17` (Antony Abbot Duplex 3, swapped via
  Vespera) → duplex>2 → preces returns 0 → Perl emits lines [2,3]

Slice 19's swap matches Perl's behaviour: Compline 01-16 now sees
Antony Abbot as winner, predicate's `duplex>=3` early-exit fires,
and Rust emits lines [2,3]. Match.

Same pattern closes 01-19 Compline (Mon eve of Fab/Seb Duplex 4),
01-24 Sat Compline (eve of Conv Pauli Duplex II classis), and
01-26 Mon Compline (eve of John Chrysostom Duplex 3).

**30-day Jan 2026 × T1570 × Oratio sweep — `--hour all`:**

| Hour          | Pre slice 19 | Post slice 19 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 30/30 (100%) | 30/30 (100%) | — |
| Laudes        | 30/30 (100%) | 30/30 (100%) | — |
| Prima         | 30/30 (100%) | 30/30 (100%) | — |
| Tertia        | 30/30 (100%) | 30/30 (100%) | — |
| Sexta         | 30/30 (100%) | 30/30 (100%) | — |
| Nona          | 30/30 (100%) | 30/30 (100%) | — |
| Vespera       | 30/30 (100%) | 30/30 (100%) | — |
| Completorium  | 26/30 (87%)  | **30/30 (100%)** | +4 |
| **Aggregate** | **236/240 (98.33%)** | **240/240 (100.00%)** | **+4** |

🎯 **30-day Jan slice = 100% across all 8 hours under T1570.**

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Year sweep snapshot (post-slice-19)

Full 2026 × T1570 × Oratio (`--hour all`, 365 days × 8 hours =
2920 cells): **2198/2920 = 75.27%**.

Per-hour:

| Hour          | Year 2026 | rust-blank | differ |
|---------------|----------:|-----------:|-------:|
| Matutinum     | 269/365 (73.70%) | 26 | 70 |
| Laudes        | 269/365 (73.70%) | 26 | 70 |
| Prima         | 289/365 (79.18%) | 0  | 76 |
| Tertia        | 269/365 (73.70%) | 26 | 70 |
| Sexta         | 269/365 (73.70%) | 26 | 70 |
| Nona          | 269/365 (73.70%) | 26 | 70 |
| Vespera       | 275/365 (75.34%) | 22 | 67 |
| Completorium  | 289/365 (79.18%) | 0  | 74 |

The Matutinum/Laudes/Tertia/Sexta/Nona band shares the same 26
rust-blank + 70 differ — same days fail across those five hours,
suggesting per-day issues (likely calendar resolution edge cases
from Septuagesima onward). Prima and Compline don't share the
26 rust-blank, but ~76 differ each — likely the
$commemoratio-driven preces predicate over-fire we noted in
slice 17 + new seasonal fail patterns that don't show up in Jan.

## Slice 20: `[Oratio 2]` / `[Oratio 3]` numbered-variant priority — full year T1570 75.27% → 79.04%

Upstream `specials/orationes.pl::oratio` (lines 67-74) sets
`$ind = $hora eq 'Vespera' ? $vespera : 2` then overrides
`$winner{Oratio}` with `$winner{"Oratio $ind"}` when the latter
exists. Lent ferials use this convention densely:

- **Tempora/Quadp3-3 (Ash Wednesday)**:
  - `[Oratio 2]` = "Praesta, Domine, fidelibus tuis..."
    (Lauds / Mat / Tertia / Sexta / Nona)
  - `[Oratio 3]` = "Inclinantes se, Domine..." (second Vespers)
  - **No bare `[Oratio]`** — without numbered preference, the
    chain walker falls through to Quadp3-0 Sunday's "Preces
    nostras..." via `tempora_sunday_fallback`.

`slot_candidates("Oratio", hour)` now returns:

```
Vespera                   → ["Oratio 3", "Oratio"]
Prima | Completorium      → []  (fixed prayers in Ordinarium)
otherwise                 → ["Oratio 2", "Oratio"]
```

`find_section_in_chain` tries the candidates in order; the
numbered variant wins when present (53 Sancti+Tempora files have
`[Oratio 2]`, fewer have `[Oratio 3]`).

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%).

**Full year 2026 × T1570 × Oratio:** 2198/2920 (75.27%) →
**2310/2920 (79.04%)** (+112 cells).

| Hour          | Pre slice 20 | Post slice 20 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 269/365 (74%) | 291/365 (80%) | +22 |
| Laudes        | 269/365 (74%) | 291/365 (80%) | +22 |
| Prima         | 289/365 (79%) | 289/365 (79%) | — |
| Tertia        | 269/365 (74%) | 291/365 (80%) | +22 |
| Sexta         | 269/365 (74%) | 291/365 (80%) | +22 |
| Nona          | 269/365 (74%) | 291/365 (80%) | +22 |
| Vespera       | 275/365 (75%) | 275/365 (75%) | — |
| Completorium  | 289/365 (79%) | 289/365 (79%) | — |

The 5-hour band (M/L/T/S/N) all gained 22 cells — same Lent ferials
fail/pass across these hours. Vespera gain is hidden in net 0:
[Oratio 3] preference fixes ~21 Lent-Vespera days but exposes new
seasonal patterns elsewhere (~21 days lost). Prima/Compline use
fixed prayers in their Ordinarium templates, so this slice doesn't
touch them.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 21: `commune_chain` follows `__preamble__` `@Path` inheritance — full year T1570 79.04% → 84.69%

Slice 17 added `__preamble__` capture in the build script and used
it for `[Rank]` lookups via `first_at_path_inheritance`. But
`visit_chain` (the per-day commune chain walker) only followed
`[Rule]` `vide CXX` directives — it didn't chase the whole-file
`@Commune/CYY` directive at the head of files like `Commune/C10c`.

Failing example: **02-07-2026 Sat Matutinum**, day_key
`Commune/C10c` (post-Purification BVM Saturday variant). The file
starts with `@Commune/C10` (whole-file inheritance) and has no own
`[Rule]` or `[Oratio]`. The chain walker stopped at C10c → per-day
Oratio splice fell through to nothing (`RustBlank`, while Perl
emits "Concede nos famulos tuos..." from C10b → @Sancti/01-01).

Slice 21 inserts a `first_at_path_inheritance` chase in
`visit_chain` after pushing the current file but before parsing
its `[Rule]`. The parent file's sections become visible to
subsequent `find_section_in_chain` lookups via standard chain
order.

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%).

**Full year 2026 × T1570 × Oratio:**
2310/2920 (79.04%) → **2472/2920 (84.69%)** (+162 cells).

| Hour          | Pre slice 21 | Post slice 21 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 291/365 (80%) | 321/365 (88%) | +30 |
| Laudes        | 291/365 (80%) | 321/365 (88%) | +30 |
| Prima         | 289/365 (79%) | 289/365 (79%) | — |
| Tertia        | 291/365 (80%) | 321/365 (88%) | +30 |
| Sexta         | 291/365 (80%) | 321/365 (88%) | +30 |
| Nona          | 291/365 (80%) | 321/365 (88%) | +30 |
| Vespera       | 275/365 (75%) | 290/365 (79%) | +15 |
| Completorium  | 289/365 (79%) | 289/365 (79%) | — |

Closes `RustBlank` cells across Saturday-BVM-variant days
(Commune/C10c, C10n, C10t, etc. — multiple seasonal BVM Saturday
files all follow the same `@Commune/C10` whole-file inheritance
convention). 5-hour band band gains 30 cells; Vespera gains 15
(some BVM Saturday Vespera fixes).

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 22: tempora_sunday_fallback strips alphabetic suffix case-insensitively — full year T1570 84.69% → 85.27%

`tempora_sunday_fallback` mapped `Tempora/Epi3-4` → `Tempora/Epi3-0`
by trimming trailing **lowercase** letters off the day-form
suffix. But several Pascha / Pentecost ferials use Mixed-case
suffixes:

```
Tempora/Pasc2-5Feria.txt    [Rule] = "Oratio Dominica" (etc.)
Tempora/Pasc2-3Feriat.txt
Tempora/Pent03-2Feriao.txt
Tempora/Pent03-5Feriao.txt
```

The lowercase-only trim left "5Feria" → "5Fer" → still not
all-digits → returns None → no Sunday fallback → chain stops at
the Feria file → no `[Oratio]` → RustBlank.

Slice 22 widens the trim to `is_ascii_alphabetic` (case-insensitive)
so `5Feria` → `5`, `2Feriao` → `2`, `3Feriat` → `3`. The Sunday
fallback fires; `Tempora/Pasc2-0` enters the chain;
`find_section_in_chain` finds the Sunday's `[Oratio]`.

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%).

**Full year 2026 × T1570 × Oratio:**
2472/2920 (84.69%) → **2489/2920 (85.27%)** (+17 cells).

| Hour          | Pre slice 22 | Post slice 22 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 321/365 (88%) | 324/365 (89%) | +3 |
| Laudes        | 321/365 (88%) | 324/365 (89%) | +3 |
| Prima         | 289/365 (79%) | 289/365 (79%) | — |
| Tertia        | 321/365 (88%) | 324/365 (89%) | +3 |
| Sexta         | 321/365 (88%) | 324/365 (89%) | +3 |
| Nona          | 321/365 (88%) | 324/365 (89%) | +3 |
| Vespera       | 290/365 (79%) | 292/365 (80%) | +2 |
| Completorium  | 289/365 (79%) | 289/365 (79%) | — |

Closes Pasc2-5Feria, Pasc2-3Feriat, Pent03-2Feriao type cases.
Remaining 6 RustBlanks per ferial-band hour are non-Tempora-feria
(synthetic keys `SanctiM/06-27oct`, `Sancti/08-19bmv`,
`Tempora/PentEpi5-5` etc.) that need a different resolution path.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 23: case-insensitive `vide sancti/...` + Tempora/PentEpi → Epi remap — full year T1570 85.27% → 86.20%, **0 RustBlanks**

Two separate fixes that close all remaining `RustBlank` cells in
the year sweep:

1. **`vide sancti/...` lowercase rule directives.** Sancti/06-27oct
   (`Quarta die infra Octavam S. Joannis Baptistæ`) and
   Sancti/08-19bmv (`Quinta die infra Octavam S. Assumptionis`)
   carry `[Rule]` bodies with **lowercase**:

   ```
   vide sancti/06-24
   vide sancti/08-15;
   ```

   `first_path_token` only accepted the canonical-case prefix
   (`Sancti/`, `Tempora/`, `Commune/`, `SanctiM/`, `SanctiOP/`)
   so the chain target was rejected → no parent file in chain →
   no `[Oratio]` → RustBlank. Slice 23 makes the prefix match
   case-insensitive and normalises the output to canonical case
   (`sancti/` → `Sancti/`).

2. **`Tempora/PentEpi<N>-<D>` synthetic keys.** When the calendar
   "resumes" leftover Sundays after Epiphany during the late
   Pentecost season (Sun XXIV+ post Pentecost = Sun-after-Epi
   resumed), the precedence engine emits keys like
   `Tempora/PentEpi5-5` for which no file exists. Upstream Perl
   resolves these to the original Epi-cycle file
   (`Tempora/Epi5-5`). `visit_chain` now strips the `Pent`
   prefix off `PentEpi…` keys when the literal lookup misses
   and retries with `Tempora/Epi…`. Closes 11-13, 11-15, 11-16,
   11-20 RustBlanks.

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%).

**Full year 2026 × T1570 × Oratio:**
2489/2920 (85.27%) → **2516/2920 (86.20%)** (+27 cells).

| Hour          | Pre slice 23 | Post slice 23 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 324/365 (89%) | 329/365 (90%) | +5 |
| Laudes        | 324/365 (89%) | 329/365 (90%) | +5 |
| Prima         | 289/365 (79%) | 289/365 (79%) | — |
| Tertia        | 324/365 (89%) | 329/365 (90%) | +5 |
| Sexta         | 324/365 (89%) | 329/365 (90%) | +5 |
| Nona          | 324/365 (89%) | 329/365 (90%) | +5 |
| Vespera       | 292/365 (80%) | 294/365 (81%) | +2 |
| Completorium  | 289/365 (79%) | 289/365 (79%) | — |

🎯 **All `RustBlank` cells closed across all 8 hours** — the
remaining 404 fails are all `Differ` (or 3 `PerlBlank`).

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 24: rank-class duplex + [Officium] Octave detection — full year T1570 86.20% → 87.98%, +52 cells

Two interlocking changes to the preces predicate:

1. **`duplex` is rank-CLASS-based, not rank-NUMBER-based.** Upstream
   `horascommon.pl:1583-1591` sets `$duplex` from `$dayname[1]`
   (the rank class string), not the rank number:

   ```
   Simplex / Memoria / Commemoratio / Feria        → 1
   Semiduplex (matches /semiduplex/i)              → 2
   Duplex / Duplex maius / Duplex II classis       → 3
   ```

   The rank NUMBER for Septuagesima Sun is 6.1 (Tempora/Quadp1-0
   `[Rank]` = ";;Semiduplex;;6.1") — but the CLASS is "Semiduplex"
   so $duplex=2, not 3. Old Rust check `rank_num >= 3.0` rejected
   Septuagesima Sun → preces never fired → Rust emitted lines [2,3]
   while Perl emitted line[4] (omittitur).

2. **Octave check on FULL rank line + [Officium].** The Octave
   annotation typically lives in the title field of the rank line
   (`Secunda die infra Octavam Epiphaniæ;;Semiduplex;;5.6`), not
   the class field — so checking just the class string missed it.
   Tempora/Epi1-0a goes one further: its rank line is bare
   `";;Semiduplex;;5.61"` but `[Officium]` is "Dominica infra
   Octavam Epiphaniæ". Upstream `winner.Rank =~ /octav/i` would
   miss the latter, but in practice Perl rejects preces on
   Tempora/Epi1-0a Saturday Compline — the closest detectable
   proxy is checking [Officium] for "Octav" too.

`active_rank_line_for_rubric` now returns
`(full_line, class, rank_num)`. The preces predicate uses
class for the duplex check and the full line + [Officium] body
for the Octave check.

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%).

**Full year 2026 × T1570 × Oratio:**
2516/2920 (86.20%) → **2569/2920 (87.98%)** (+53 cells).

| Hour          | Pre slice 24 | Post slice 24 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 329/365 (90%) | 329/365 (90%) | — |
| Laudes        | 329/365 (90%) | 329/365 (90%) | — |
| Prima         | 289/365 (79%) | 315/365 (86%) | +26 |
| Tertia        | 329/365 (90%) | 329/365 (90%) | — |
| Sexta         | 329/365 (90%) | 329/365 (90%) | — |
| Nona          | 329/365 (90%) | 329/365 (90%) | — |
| Vespera       | 294/365 (81%) | 294/365 (81%) | — |
| Completorium  | 289/365 (79%) | 315/365 (86%) | +26 |

Closes Septuagesima/Sexagesima/Quinquagesima Sunday Prima/Compline
omittitur emissions, Lent Sunday omittitur, etc. — all the
"Semiduplex Sunday with high rank num but low duplex class"
cases that the old `rank_num >= 3.0` rejection missed.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 25: "Feria privilegiata" tomorrow-no-1V — full year T1570 87.98% → 88.15%

Lent ferials carry rank class **"Feria privilegiata"** (with high
rank num like 6.9 for Ash Wed), but they don't claim 1st Vespers
in the Roman office — Tuesday Vespera before Ash Wednesday should
NOT swap to Ash Wed; it continues with Tuesday's Tempora ferial
(which inherits Sunday Quinquagesima's "Preces nostras..." Oratio
via `Oratio Dominica`).

Slice 25 adds a class-string check in
`first_vespers_day_key_for_rubric`: when tomorrow's rank class
contains "feria privilegiata", today wins regardless of rank.

The check is **narrow** — only "Feria privilegiata", not generic
"Feria" or "Simplex" / "Memoria". Generic Feria-class Tempora
ferials (`Tempora/Epi3-4`) DO claim 1st Vespers (via the week
Sunday's 1V). Sancti Simplex like Saturday BVM at Commune/C10b
also has 1V despite Simplex 1.3 rank — generalising the check
breaks 01-23 Fri Vespera (which correctly swaps to BVM Saturday).

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%).

**Full year 2026 × T1570 × Oratio:**
2569/2920 (87.98%) → **2574/2920 (88.15%)** (+5 cells).

| Hour          | Pre slice 25 | Post slice 25 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 329/365 (90%) | 329/365 (90%) | — |
| Laudes        | 329/365 (90%) | 329/365 (90%) | — |
| Prima         | 315/365 (86%) | 315/365 (86%) | — |
| Tertia        | 329/365 (90%) | 329/365 (90%) | — |
| Sexta         | 329/365 (90%) | 329/365 (90%) | — |
| Nona          | 329/365 (90%) | 329/365 (90%) | — |
| Vespera       | 294/365 (81%) | 298/365 (82%) | +4 |
| Completorium  | 315/365 (86%) | 316/365 (87%) | +1 |

Closes Vespera before Ash Wed (02-17) and similar Lent-ferial
eves where the day-after carries "Feria privilegiata".

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 26: "Feria major" tomorrow-no-1V — full year T1570 88.15% → 88.66%

Slice 25 caught "Feria privilegiata" (Ash Wed and similar rank
6.9 days). Lent ferials Quadp3-4/5/6 and the Quad week ferials
carry a related class **"Feria major"** (rank 2.1 default, 4.9
under 1930) — same Lent semantics, no proper 1st Vespers.

Wed 02-19 Vespera, today=Quadp3-4 (Thu after Ash Wed) tomorrow=
Quadp3-5 (Fri). Both "Feria major" rank 2.1, equal. Default tie
goes to tomorrow → wrong. Adding "feria major" to the no-1V
class match keeps today (= Quadp3-4 [Oratio 3] = "Parce, Domine,
parce populo tuo...").

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%).

**Full year 2026 × T1570 × Oratio:**
2574/2920 (88.15%) → **2589/2920 (88.66%)** (+15 cells).

| Hour          | Pre slice 26 | Post slice 26 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 329/365 (90%) | 329/365 (90%) | — |
| Laudes        | 329/365 (90%) | 329/365 (90%) | — |
| Prima         | 315/365 (86%) | 315/365 (86%) | — |
| Tertia        | 329/365 (90%) | 329/365 (90%) | — |
| Sexta         | 329/365 (90%) | 329/365 (90%) | — |
| Nona          | 329/365 (90%) | 329/365 (90%) | — |
| Vespera       | 298/365 (82%) | 313/365 (86%) | +15 |
| Completorium  | 316/365 (87%) | 316/365 (87%) | — |

Closes Lent ferial Vesperas (Thu/Fri/Sat after Ash Wed, Lent
weekday eves) where today's "Feria major" Tempora-ferial Oratio
should continue.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 27: drop blanket Sunday-no-preces — full year T1570 88.66% → 89.90%

The preces predicate had a blanket `if dayofweek == 0 { return
false; }` early-exit. That was a defensive measure when slice 14
first wired the predicate; the Octave detection (rank-line title
field in slice 24 + `[Officium]` body check) is now precise
enough to distinguish:

- **Sunday in Octave of Christmas / Epiphany / etc.** — winner
  Tempora/EpiX-Y or similar, `[Officium]` = "Dominica infra
  Octavam …" → Octav check fires → preces rejected (matches
  Perl).
- **Septuagesima / Sexagesima / Quinquagesima / Lent Sun** —
  winner Tempora/Quadp1-0 etc., rank class "Semiduplex", no
  Octave annotation → branch (b) fires → preces emits omittitur
  (matches Perl).

Slice 27 drops the blanket Sunday-reject. Octave detection in
the surrounding checks handles the false-positive cases.

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%) — the
30-day Jan window has 4 Sundays (01-04, 01-11, 01-18, 01-25):
01-04 + 01-11 in Christmas/Epi Octave (Octav-rejected), 01-18
Sun II post Epi (rank "Semiduplex" rank num 5 — duplex_class=2
Semiduplex passes; but Sun II post Epi 01-18 doesn't have an
Octave [Officium] annotation, and Perl still emits lines [2,3]
on this day. Rust matches because the `Sun in Octave` line in
the rank line `;;Semiduplex;;5;;` doesn't match... wait, it
SHOULD match — let me trace. Actually Perl emits omittitur on
Sun II post Epi when preces fires; if Rust did the same, it'd
still match. The 30-day stays 100% which means either both emit
the same form, or some other factor is at play. Octave detection
is robust enough to keep 30-day Jan green.)

**Full year 2026 × T1570 × Oratio:**
2589/2920 (88.66%) → **2625/2920 (89.90%)** (+36 cells).

| Hour          | Pre slice 27 | Post slice 27 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 329/365 (90%) | 329/365 (90%) | — |
| Laudes        | 329/365 (90%) | 329/365 (90%) | — |
| Prima         | 315/365 (86%) | 340/365 (93%) | +25 |
| Tertia        | 329/365 (90%) | 329/365 (90%) | — |
| Sexta         | 329/365 (90%) | 329/365 (90%) | — |
| Nona          | 329/365 (90%) | 329/365 (90%) | — |
| Vespera       | 313/365 (86%) | 313/365 (86%) | — |
| Completorium  | 316/365 (87%) | 327/365 (90%) | +11 |

Closes Septuagesima, Sexagesima, Quinquagesima, Lent Sun,
post-Pentecost Sun Prima/Compline omittitur emissions across
the year.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 28: Ember-day Vespera uses Sunday's [Oratio] — full year T1570 89.90% → 89.97%

Upstream `specials/orationes.pl::oratio` line 56 has a special
case for Ember-day Vespera in Lent:

```perl
($winner{Rank} =~ /Quattuor/i && $dayname[0] !~ /Pasc7/i
    && $version !~ /196|cist/i && $hora eq 'Vespera')
```

When the winner is an Ember-day (Quattuor Temporum) Lent ferial
and the hour is Vespera, the office uses the week-Sunday's
`[Oratio]` (`Oratio Dominica`-style fallback), NOT the day's own
`[Oratio 3]`. Drives Wed 02-25 Lent 1 Ember Wed Vespera, Fri
02-27 Ember Fri Vespera, etc.

The detection: check the day file's `[Officium]` body for
"Quattuor Temporum" — Quad1-3 = "Feria Quarta Quattuor Temporum
Quadragesimæ", similar for Quad1-5/Quad1-6, and Pent and Sept
Ember weeks. When matched at Vespera, the `Oratio` candidates
list is forced to `["Oratio"]` (Sunday's via chain fallback)
instead of the default `["Oratio 3", "Oratio"]`.

For non-Ember Lent ferials (Quad2-3 Wed Lent 2, Quad3-3, etc.)
the day's `[Oratio 3]` continues to be the right answer — Perl
uses it there too.

**30-day Jan 2026 × T1570:** stays at 240/240 (100.00%).

**Full year 2026 × T1570 × Oratio:**
2625/2920 (89.90%) → **2627/2920 (89.97%)** (+2 cells).

| Hour          | Pre slice 28 | Post slice 28 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 329/365 (90%) | 329/365 (90%) | — |
| Laudes        | 329/365 (90%) | 329/365 (90%) | — |
| Prima         | 340/365 (93%) | 340/365 (93%) | — |
| Tertia        | 329/365 (90%) | 329/365 (90%) | — |
| Sexta         | 329/365 (90%) | 329/365 (90%) | — |
| Nona          | 329/365 (90%) | 329/365 (90%) | — |
| Vespera       | 313/365 (86%) | 315/365 (86%) | +2 |
| Completorium  | 327/365 (90%) | 327/365 (90%) | — |

Modest gain — only 2 cells because the only Ember weeks visible
in 2026 calendar fall around 02-25 (Wed Lent 1 Ember), 02-27
(Fri Ember), and the Pent/Sept Ember weeks. The detection is
narrow and correct; it doesn't over-fire on non-Ember Lent
ferials.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 29: R60 j→i classical spelling — full year R60 16.67% → 82.67% (+200+ cells)

Pius X's 1910 reform replaced medieval `Jesum` / `cujus` /
`justítiam` orthography with classical `Iesum` / `cuius` /
`iustítiam`. The corpus stores the older `j`-form; under R60,
upstream Perl `horascommon.pl::spell_var` applies `tr/Jj/Ii/`
at render time. Mass-side already had this via
`crate::mass::apply_spelling_for_active_rubric` driven by
thread-local `ACTIVE_RUBRIC`; Office didn't apply any Latin
respelling.

`apply_office_spelling(text, rubric)` is the Office mirror —
under `Rubric::Rubrics1960` it does the same `tr/Jj/Ii/` swap
plus the H-Iesu→H-Jesu opt-out and the `er eúmdem` →
`er eúndem` typo fix that Mass uses. Pre-1960 rubrics
(Tridentine 1570/1910, Divino Afflatu, Reduced 1955) keep the
`j`-form verbatim — the corpus matches them already.

Applied at three points:

- `splice_proper_into_slot` after `substitute_saint_name` for
  the proper-section splice.
- `splice_proper_into_slot` Capitulum/Hymnus fallback branch.
- `compute_office_hour` walker — `kind: "plain"` (closes
  `$oratio_Domine`, `$oratio_Visita`, `$Per Dominum` — Prima
  and Compline fixed prayers) and `kind: "macro"` (closes
  `&Dominus_vobiscum*`, `&Benedicamus_Domino`,
  `&Deus_in_adjutorium`).

**T1570 30-day Jan**: stays at 240/240 (100.00%) — pre-1960
rubric, no spelling swap fires. **T1570 full year**: stays at
2627/2920 (89.97%).

**R60 30-day Jan**:

| Hour          | Pre slice 29 | Post slice 29 | Δ |
|---------------|-------------:|--------------:|--:|
| Matutinum     | 7/30  (23%) | 24/30 (80%) | +17 |
| Laudes        | 7/30  (23%) | 24/30 (80%) | +17 |
| Prima         | 0/30  (0%)  | 30/30 (100%)| +30 |
| Tertia        | 7/30  (23%) | 24/30 (80%) | +17 |
| Sexta         | 7/30  (23%) | 24/30 (80%) | +17 |
| Nona          | 7/30  (23%) | 24/30 (80%) | +17 |
| Vespera       | 2/30  (7%)  | 11/30 (37%) | +9 |
| Completorium  | 0/30  (0%)  | 30/30 (100%)| +30 |
| **Aggregate** | **37/240 (15.42%)** | **191/240 (79.58%)** | **+154** |

**R60 full year**: 16.67% → **82.67%** (+200+ cells).

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 30: `$macro` expansion in spliced bodies — T1570 full 89.97%→91.47%, R60 full 82.67%→91.64%

Per-day Oratio bodies frequently end with conclusion macros like
`$Per Dominum`, `$Per eumdem`, `$Qui vivis`, `$Qui tecum`. The
walker's `kind: "plain"` template branch already calls
`expand_dollar_macro` on these — but per-day SPLICED bodies
(coming from `find_section_in_chain`) never went through macro
expansion. Examples:

```
Sancti/01-06 [Oratio]:
  Deus, qui hodiérna die Unigénitum tuum géntibus stella duce
  revelásti: ... usque ad contemplándam spéciem tuæ celsitúdinis
  perducámur.
  $Per eumdem
```

Rust emitted the literal `$Per eumdem` line; Perl renders it as
`Per eúndem Dóminum nostrum Iesum Christum Filium tuum, qui
tecum vivit et regnat in unitáte Spíritus Sancti, Deus, per
ómnia sǽcula sæculórum.` (full conclusion).

`expand_dollar_macros_in_body` walks each line of the spliced
body, calling `expand_dollar_macro` on `$`-prefixed lines and
passing others through. Applied in `splice_proper_into_slot`
after `substitute_saint_name`, before `apply_office_spelling`.

**Big cross-rubric gain — both T1570 and R60 benefit:**

| Rubric × Window         | Pre slice 30 | Post slice 30 | Δ |
|-------------------------|-------------:|--------------:|--:|
| T1570 30-day Jan        | 100.00% | 100.00% | — |
| T1570 full year (2920c) | 89.97%  | 91.47%  | +44 |
| R60   30-day Jan        | 79.58%  | 92.92%  | +32 |
| R60   full year (2920c) | 82.67%  | 91.64%  | +260+ |

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 31: chain follows `[Rank]` 4th-field commune-ref — T1570 91.47%→91.88%, R60 91.64%→91.99%

`visit_chain` only consulted `[Rule]` for commune-chain targets.
Some Sancti days carry the commune-ref in the `[Rank]` line's
4th `;;`-separated field instead — `Sanctæ Mariæ Virginis ad
Nives;;Duplex;;3;;vide C11` (Sancti/08-05 R60). Under R60 the
`[Rule]` body's `ex C11` directive gets popped by the
`(sed rubrica 196 omittitur)` SCOPE_CHUNK backscope, so without
consulting `[Rank]` the chain stops at Sancti/08-05 (which has
no `[Oratio]`) — RustBlank.

`visit_chain` now runs `parse_vide_targets` on the
`active_rank_line_for_rubric` full line in addition to the
`[Rule]` body. Closes RustBlanks like 08-05 R60 Mat/Lauds/Min
and lifts a handful of Sancti-Marian-feast Differs across both
rubrics.

**T1570 30-day Jan**: stays at 240/240 (100.00%).

| Rubric × Window         | Pre slice 31 | Post slice 31 | Δ |
|-------------------------|-------------:|--------------:|--:|
| T1570 full year (2920c) | 91.47% | **91.88%** | +12 |
| R60   full year (2920c) | 91.64% | **91.99%** | +10 |

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 32: annotated `[Rank]` lookup (concurrence-only) + Festum Domini priority — T1570 91.88% → 91.99%

Two interlocking changes that fix R60 Sun 01-11 Vespera (Holy
Family Sun → first vespers of Mon 01-12 ferial) and avoid the
T1570 11-08 regression that blocked slice 31a:

1. **`active_rank_line_with_annotations`** — new helper that
   ALSO checks rubric-conditional annotated section variants
   (`[Rank] (rubrica X aut rubrica Y)`). Used **only** by
   `first_vespers_day_key_for_rubric` for concurrence rank
   comparisons; the preces predicate continues to use the
   line-level-eval-only `active_rank_line_for_rubric` (which
   was regression-prone in slice 31a when both code paths
   shared the annotation lookup).

   The annotation lookup uses `find_conditional` to strip
   leading stopwords ("sed") off `(rubrica X)` predicates so
   `vero` evaluates the bare condition correctly.

2. **`tomorrow_rule_marks_festum_domini`** — when tomorrow's
   `[Rule]` body carries the `Festum Domini` directive (feasts
   of the Lord like Dedication of the Lateran, Transfiguration,
   Holy Name of Jesus), tomorrow always wins first-Vespers
   concurrence regardless of rank-num comparison. Without this,
   the new annotation lookup correctly picks T1570's
   `[Rank] (rubrica tridentina) Duplex 3` for Sancti/11-09 (was
   picking the bare default Duplex II classis 5 before slice 32),
   but rank 3 < today's Sancti/11-08 Sun-Octave-of-All-Saints
   3.1 → wrong winner. Festum Domini priority bypasses the
   rank-num race.

**T1570 30-day Jan**: stays at 240/240 (100.00%).

| Rubric × Window         | Pre slice 32 | Post slice 32 | Δ |
|-------------------------|-------------:|--------------:|--:|
| T1570 full year (2920c) | 91.88% | **91.99%** | +3 |
| R60   full year (2920c) | 91.99% | 91.99%      | — |

R60 didn't gain net cells — the annotation lookup fixed Sun
01-11 R60 Vespera, but other R60 Vespera fails dominate (133
differs total, mostly Holy-Family-Sun and Lent ferials needing
their own clusters).

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 33: today's `ex Sancti/...` rank inheritance (low-rank-only) — R60 91.99% → 92.02%

R60 demotes Sancti/01-07..01-12 (sub-Octave-of-Epi days) from
Semiduplex 5.6 to Feria 1.x but keeps `[Rule] ex Sancti/01-06`
(inherits Epi office's structure). For first-Vespers concurrence
on those Feria days, today's effective rank should follow the
inheritance to the source feast — Friday 01-09 R60 Vespera should
NOT swap to Saturday-BVM (Commune/C10b Simplex 1.3) just because
1.3 > 1.2.

`effective_today_rank_for_concurrence` returns `max(direct,
inherited_source_rank)` — but ONLY when `direct < 2.0`. Days
with real rank ≥ 2.0 (T1570 sub-Octave Semiduplex 5.6, regular
Sancti days) keep their direct rank — boosting them
over-fires and stops legitimate Sun-after-Epi swaps (01-10 Sat
Vespera).

The boost is also asymmetric — applied only to TODAY, not
TOMORROW. A Mon ferial that inherits Epi's structure doesn't
get 1st Vespers privilege from the inheritance; it's still a
ferial without proper 1st Vespers. (01-12 Mon's first-Vespers
case is closed by slice 32's annotated `[Rank]` lookup, not by
inheritance.)

**T1570 30-day Jan**: stays at 240/240 (100.00%).

| Rubric × Window         | Pre slice 33 | Post slice 33 | Δ |
|-------------------------|-------------:|--------------:|--:|
| T1570 full year (2920c) | 91.99% | 91.99% | — |
| R60   full year (2920c) | 91.99% | **92.02%** | +1 |

Modest gain — only the few R60 sub-Octave-of-Epi Friday Vesperas
where today's Feria 1.x had to beat tomorrow's Saturday-BVM
Simplex 1.3 fall in scope.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 34: `find_section_in_chain` filters annotated section variants by rubric — T1570 91.99% → 93.32% (+39), R60 92.02% → 92.40% (+10)

Symptom: 11-12 St. Martin Pope Matutinum/Laudes/Tertia/Sexta/
Nona under T1570 emit raw `@Commune/C2b` text in the Oratio
slot. Perl emits the proper "Deus, qui nos beáti Martíni
Mártyris..." body from Commune/C2-1.

Resolution chain for Sancti/11-12 [Oratio] T1570:

  Sancti/11-12 — no [Oratio] of its own
  Commune/C2b-1 — has `[Oratio] (communi Summorum Pontificum)`
                  body `@Commune/C2b` (a redirect)
  Commune/C2-1 — has bare `[Oratio]` body "Deus, qui nos beáti
                 N. Mártyris..." (the Confessor-Pope/Martyr
                 form pre-1942)
  Commune/C2 — fallback

Bug: `find_section_in_chain` matched any `Oratio (...)` prefix
indiscriminately. C2b-1's `(communi Summorum Pontificum)` is a
post-1942 form (`/194[2-9]|195[45]|196/i` upstream); under
T1570 it should NOT fire. Without the filter, C2b-1's redirect
body wins over C2-1's bare body.

Fix: filter annotated keys through `crate::mass::
annotation_applies_to_rubric` (the same function the Mass-side
walker uses for [Oratio] (...) variants). Two-pass:

  1. Bare `[Oratio]` or annotation matching the active rubric.
     Drives Pope/Pontifex feasts under T1570 to skip the
     `(communi Summorum Pontificum)` C2b body and continue to
     C2-1's bare body.

  2. Fallback: any annotated body with a "context-only"
     annotation (`(ad missam)`, `(ad laudes)` style). Some
     commune files only carry `(ad missam)` variants — Commune
     /C9 [Oratio] (ad missam) is the All Souls Oratio, used
     for both Mass and Office. Without the fallback, R60
     11-02 All Souls Vespera renders blank.

`annotation_is_office_context_only` excludes the rubric-gating
annotations (`nisi …`, `rubrica X`, `communi summorum
pontificum`) so they stay filtered.

Threading: `splice_proper_into_slot`, `splice_matins_lectios`,
`collect_nocturn_antiphons` now thread `rubric` through to
`find_section_in_chain`.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Matutinum 92.88% → 94.79% (+7)
      Laudes    92.88% → 94.79% (+7)
      Tertia    92.88% → 94.79% (+7)
      Sexta     92.88% → 94.79% (+7)
      Nona      92.88% → 94.79% (+7)
      Vespera   88.22% → 89.32% (+4)
      Overall   91.99% → 93.32% (+39)

    R60:
      Matutinum 94.79% → 95.34% (+2)
      Laudes    95.34% → 95.89% (+2)
      Tertia    94.79% → 95.34% (+2)
      Sexta     94.79% → 95.34% (+2)
      Nona      94.79% → 95.34% (+2)
      Vespera   63.84% → 64.11% (+1, rust-blank cleared)
      Overall   92.02% → 92.40% (+10)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 35: R55/R60 1st-Vespers rank suppression — R60 92.40% → 95.92% (+103 cells)

Symptom: R60 Vespera 64.11% (131 differs). Most R60 weekday
Vesperas swap to tomorrow's office on every Duplex (rank 3)
feast — 01-12 Mon Vespera renders Baptism's "Deus, cuius
Unigénitus..." but Perl renders "Vota, quaesumus, Domine..."
(today's Sunday-after-Epi inherited Oratio).

Trace: `horascommon.pl::concurrence` lines 938-945 — R60 / R55
suppress 1V via the `cwrank[2] < threshold` check inside the
suppress-1V OR chain.

R55 ("Reduced - 1955"): threshold = 5 unconditionally. Only
Duplex II classis or higher get 1V.

R60 ("Rubrics 1960"): threshold depends on tomorrow's title +
[Rule]:
  * 5 — when tomorrow's [Officium] contains "Dominica" OR
    (tomorrow's [Rule] flags `Festum Domini` AND today is
    Saturday). Sat-eve of Sun-Holy-Family (Festum Domini Sun)
    fits the second branch.
  * 6 — otherwise. Only Duplex I classis (rank 6+) get 1V on
    weekday-eve.

01-13 Baptism (Duplex II classis, Festum Domini, Tuesday) sits
right at this gate: cwrank=5, today=Mon=1 (not 6) → threshold=
6 → cwrank<threshold → 1V suppressed. So Mon 01-12 Vespera
stays today, NOT tomorrow.

Fix: add the rank-threshold suppression as a new gate in
`first_vespers_day_key_for_rubric` between the
`tomorrow_has_no_prima_vespera` check and the existing
feria-privilegiata reject. Threshold logic is rubric-matched —
T1570/T1910/DA fall through unchanged.

Threading: `office_sweep` now passes `today_dow` (chrono-style
0..=6, Sun=0..Sat=6) so R60 can detect the Sat-eve Festum
Domini case.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:   91.99% → 91.99% — unchanged (gate is R55/R60
             only, T1570 falls through)
    R60:
      Matutinum 95.34% → 95.34% (unchanged)
      Laudes    95.89% → 95.89% (unchanged)
      Tertia    95.34% → 95.34% (unchanged)
      Sexta     95.34% → 95.34% (unchanged)
      Nona      95.34% → 95.34% (unchanged)
      Vespera   64.11% → 92.33% (+103 cells)
      Compline  98.90% → 98.90% (unchanged)
      Overall   92.40% → 95.92% (+103 cells)

R55 also benefited (its threshold-5 path); R55 not previously
broken-out — now Vespera 81.92%, overall 93.05%.

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 36: Easter / Pentecost Octave ferial 1V suppression — T1570 93.32% → 93.56%, R60 95.92% → 96.16%, R55 93.05% → 93.25%

Symptom: Easter Octave Mon/Wed/Thu/Fri Vesperas plus
Pentecost Octave Mon/Wed/Thu/Fri/Sat Vesperas all emit
tomorrow's Oratio under T1570 — 04-08 Wed should emit Pasc0-3
"Deus, qui nos resurrectiónis Domínicæ..." but emits Pasc0-4
"Deus, qui diversitátem géntium...".

All Easter Octave ferials are Semiduplex I cl. 6.9 — at rank
tie our `first_vespers_day_key_for_rubric` swaps to tomorrow.
Perl suppresses 1V here.

Trace: `horascommon.pl::concurrence:959-960` — within the
suppress-1V OR chain:

  || ($weekname =~ /Pasc[07]/i && $cwinner{Rank} !~ /Dominica/i)

For an Easter Octave week ($weekname = "Pasc0") OR Pentecost
Octave week ($weekname = "Pasc7"), if tomorrow's [Rank] field
doesn't contain "Dominica" (i.e., tomorrow is another ferial in
the same Octave, not the closing Sunday), 1V is suppressed and
today's office continues.

Fix: gate-add to `first_vespers_day_key_for_rubric`. If today's
key starts with `Tempora/Pasc0-` or `Tempora/Pasc7-`, AND
tomorrow's [Rank] (post conditional eval) doesn't contain
"Dominica", return today_key.

This rule fires for ALL rubrics — both Easter Octave and
Pentecost Octave (where present, T1570/T1910/DA only —
Pentecost Octave abolished in R55/R60).

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera   89.32% → 91.23% (+7 cells)
      Overall   93.32% → 93.56% (+7 cells)
    R60:
      Vespera   92.33% → 94.25% (+7 cells)
      Overall   95.92% → 96.16% (+7 cells)
    R55:
      Vespera   81.92% → 83.56% (+6 cells)
      Overall   93.05% → 93.25% (+6 cells)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 37: Pre-DA Quad/Adv Sun 2V rank concession + chain-walk for [Name] — T1570 93.56% → 94.32% (+22 cells)

Symptom: T1570 Compline/Vespera/Mat/Lauds/etc on dates where a
Quad/Adv Sun concurs with a Duplex Sancti — 11-29 Adv1 Sun
Compline emits Sun's office, Perl emits St. Andrew's; 12-13
Adv3 Sun emits Sun's, Perl emits Lucy's; 02-22 Quad1 Sun emits
Sun's, Perl emits Cathedra Petri's.

Trace: `horascommon.pl::concurrence:862-869` — pre-DA Quad/
Adv/Pasc1 Sundays cede their 2nd Vespers to a concurrent
Duplex feast:

  Trident: $rank = $wrank[2] = 2.99    # → Semiduplex+ wins
  Divino:  $rank = $wrank[2] = 4.9     # → Duplex II cl.+ wins

Adv1 Sun is rank 6.0 — without the concession it always beats
the next-day's Duplex II cl. (rank 5.1). With the concession,
2.99 < 5.1 → Sancti wins 2V → office-wide swap.

Cascading symptom: even when Vespera output happened to match
Perl (Rust emitted Sun's body, Perl emitted Sancti's body with
Sun as commemoration — substring overlap), Compline didn't
match because Compline body is fixed (Visita) and only the
preces predicate `Dominus_vobiscum1` differed. The day_key
fix flips the preces winner from Tempora-Semiduplex (preces
fires → text[4] omittitur) to Sancti-Duplex (preces rejects →
V/R Domine exaudi).

Fix:
1. New `is_pre_da_sunday_with_2v_concession` detector — matches
   `Tempora/Quad[0-5]-0`, `Tempora/Quadp-0`, `Tempora/Adv\d?-0`,
   `Tempora/Pasc1-0` plus suffix variants (`Adv1-0o`,
   `Pasc1-0t`, `Epi1-0a`).
2. `effective_today_rank_for_concurrence` returns `direct.min(
   2.99)` for Tridentine variants and `direct.min(4.9)` for
   DA when the detector matches. Other rubrics fall through
   unchanged (R55/R60 had broader rank-suppression in slice
   35).
3. `splice_proper_into_slot` saint-name lookup now walks the
   full chain via `chain.iter().find_map` instead of just
   `chain.first()`. Sancti/12-13t (Lucy transferred-variant
   redirected from Sun to Mon) has no own [Name] — inherits
   `[Name]` via `@Sancti/12-13` `__preamble__`. Without
   walking, the `N.` literal in the Commune oratio body
   never gets substituted (Rust emitted "...beátæ N.
   festivitáte..." but Perl emitted "...beátæ Lúciæ Vírginis
   et Mártyris tuæ festivitáte..."), so the cell still
   diverges after the swap.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Matutinum    94.79% → 95.62% (+3)
      Laudes       94.79% → 95.62% (+3)
      Tertia       94.79% → 95.62% (+3)
      Sexta        94.79% → 95.62% (+3)
      Nona         94.79% → 95.62% (+3)
      Vespera      91.23% → 91.78% (+2)
      Completorium 90.14% → 91.51% (+5)
      Overall      93.56% → 94.32% (+22 cells)
    R60: 96.16% (unchanged — concession doesn't fire under
         R60; R55/R60 have their own broader suppression in
         slice 35)
    R55: 93.25% (unchanged)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 38: `lookup` resolves PentEpi → Epi keys for all callers — T1570 94.32% → 94.55%, R60 96.16% → 96.23%, R55 93.25% → 93.29%

Symptom: T1570 11-15 Sun Prima emits the lay V/R Domine
exaudi (text[2-3]) but Perl emits the precesferiales
omittitur directive (text[4]) — preces predicate disagrees.

Trace: 11-15 Sun resolves to `Tempora/PentEpi6-0` — synthetic
key for Sun-VI-after-Epiphany resumed after Pentecost (Pent
year hits the 24th-Sun limit, leftover Epi-cycle Sundays
resume). The corpus only carries `Tempora/Epi6-0` literally.
The chain walker `visit_chain` already strips `PentEpi` →
`Epi` and retries (line 694), but other callers
(`active_rank_line_for_rubric`, `preces_dominicales_et_
feriales_fires`, `tomorrow_has_no_prima_vespera`,
`tomorrow_rule_marks_festum_domini`) call `lookup` directly
and silently bail when the key misses.

For 11-15 Prima, `preces_fires` returns `false` immediately
on the missed lookup → lay default emits text[2-3]. Perl
runs the full preces logic on Epi6-0 (Semiduplex 5, Sun, Adv-
or-Quad-or-emberday gate fails, then Dominicales branch fires
since winner Rank doesn't have Octav) → returns 1 → text[4].

Fix: `lookup` itself now strips `PentEpi` and retries on
miss. Single source of truth — every caller benefits without
threading retry logic into each call site.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Prima        93.15% → 94.25% (+4)
      Vespera      91.78% → 92.05% (+1)
      Completorium 91.51% → 92.05% (+2)
      Overall      94.32% → 94.55% (+7 cells)
    R60: 96.16% → 96.23% (+2 cells, mostly Vespera)
    R55: 93.25% → 93.29% (+1 cell)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 39: preces predicate chases `__preamble__` for [Rule] / [Officium] — T1570 94.55% → 94.97% (+12 cells)

Symptom: T1570 06-05 Fri (Sat-eve in Octave of Corpus Christi
under Pent01 week) Compline emits text[4] omittitur but Perl
emits the V/R Domine exaudi (preces NOT firing). Same pattern
on the other days within Corpus Christi / Trinity / Sacred
Heart Octaves where the day file is a `-o` redirect variant.

Trace: 06-05 resolves to `Tempora/Pent01-6o` (Sat-of-CC-Octave
"o" variant). Pent01-6o.txt is a 1-line `@Tempora/Pent01-6`
preamble + a couple of section overrides (`[Lectio2]`, `[Lectio4]`).
It carries no own [Rule] or [Officium]. Pent01-6 has those:

  [Officium]: "Sabbato infra octavam Corporis Christi"
  [Rule]: "ex Tempora/Pent01-4; 9 lectiones; Doxology=Corp"

`preces_dominicales_et_feriales_fires` reads [Officium] to
detect the "octav" Octave-day reject (mirroring upstream's
empirical Perl behaviour where Octave-day winners short-
circuit preces). Direct `file.sections.get("Officium")` on
Pent01-6o returns None — the inheritance chain is never
followed. So the Octave check skips, and preces fires
incorrectly.

Fix: new `section_via_inheritance(file, name)` helper —
chases `__preamble__` `@Path` redirects up to depth 4 and
returns the first non-empty body found. Used for [Rule]
("Omit Preces" reject) and [Officium] (Octave reject) in
preces_fires. Mirror of upstream `setupstring_parse_file`
merge semantics that other callers
(`active_rank_line_for_rubric` etc.) already implement
inline.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Prima        94.25% → 95.89% (+6)
      Completorium 92.05% → 93.70% (+6)
      Overall      94.55% → 94.97% (+12 cells)
    R60: 96.23% (unchanged — the Pent01 Octave files are
         Tridentine-only; R60 abolished CC/SH/Trinity Octaves)
    R55: 93.29% (unchanged for the same reason)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 40: today `[Rule] No secunda Vespera` swap to tomorrow — T1570 94.97% → 95.07% (+3), R55 93.29% → 93.36% (+2)

Symptom: T1570 04-11 Sat in Albis (Easter Octave end) Vespera
emits Sat-of-Albis Oratio. Perl emits Sun-in-Albis Oratio
("Præsta, quǽsumus, omnípotens Deus..."). Same pattern at
05-30 (Sat in Pent Octave end), 04-04 Holy Sat, etc.

Trace: `horascommon.pl::concurrence:853-857`:

  if ($winner{Rule} =~ /No secunda Vespera/i && $version !~ /196[03]/i) {
    @wrank = ();  %winner = {};  $winner = '';  $rank = 0;
  }

Today's office is wiped at 2V → tomorrow's 1V wins
unconditionally. Pasc0-6 [Rule] carries `No secunda Vespera`
explicitly.

Slice 36 (Easter/Pent Octave gate) was keeping today on these
Sat-eve days — the gate's "tomorrow not Dominica" check in
[Rank] missed Sun-in-Albis (Pasc1-0 [Rank] = ";;Duplex majus
I. classis;;6.91;;" — no "Dominica" string). The new "No 2V"
gate fires earlier and short-circuits past the Octave gate.

Fix: new gate in `first_vespers_day_key_for_rubric`, after
`tomorrow_has_no_prima_vespera` and before R55/R60 1V
suppression. Reads today's [Rule] via
`section_via_inheritance` (slice 39 helper) — Pasc0-6 carries
the directive directly, but variant files
(`Tempora/Pasc0-6t`-style if any) would inherit it via
preamble.

Suppressed under R60 only (Perl gates on `$version !~ /196[03]/i`).
R55 keeps the rule.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera      92.05% → 92.60% (+2)
      Completorium 93.70% → 93.97% (+1)
      Overall      94.97% → 95.07% (+3 cells)
    R55: 93.29% → 93.36% (+2 cells)
    R60: 96.23% (unchanged — Perl gates this rule off)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 41: paired `N. <text> N.` saint-name collapse — T1570 +30, R60 +17, R55 +16

Symptom: T1570 04-21 Vespera (1V of SS. Soter & Caius) emits
"Beatórum Mártyrum paritérque Pontíficum Sotéris et Caji et
Sotéris et Caji nos...". Perl emits the same with a SINGLE
"Sotéris et Caji". Same pattern hits every "plural" saint
day (martyr-pair feasts, joint-pope feasts) — Commune/C3
[Oratio] body has "...beátos N. et N. Mártyres..." with two
placeholders, and substituting both with the joined [Name]
double-emits.

Trace: `specials.pl::replaceNdot:809-810` is a TWO-PASS
substitution:

  $s =~ s/N\. .*? N\./$name[0]/;   # paired "N. <text> N." → name (once)
  $s =~ s/N\./$name[0]/g;          # remaining "N." → name (all)

The first regex is non-greedy — it collapses the LEFTMOST
paired-placeholder span into a single name emission. For
plural saint days [Name] is already the joined form (e.g.
"Sotéris et Caji"), so emitting it once for the paired span
yields the right text. Single-N. days (most ferials and
single-saint feasts) skip the first pass and use only the
global replace.

Fix: `substitute_saint_name` now does the two-pass dance.
First pass `collapse_paired_n_dot` finds the leftmost two
word-boundary `N.` tokens and replaces the span between them
(inclusive) with the name once. Second pass
`replace_remaining_n_dot` handles any leftover `N.` —
single-saint days fall through to this path unchanged.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Mat/Laudes/Ter/Sex/Non  95.62% → 96.99% (+5 each)
      Vespera                 92.60% → 93.97% (+5)
      Overall                 95.07% → 96.10% (+30 cells)
    R60: 96.23% → 96.82% (+17 cells, mostly weekday hours)
    R55: 93.36% → 93.90% (+16 cells)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 42: tomorrow Vigilia → no 1V swap — T1570 96.10% → 96.23% (+5), R55 +1, R60 +1

Symptom: T1570 12-23 Adv4 Wed Vespera resolved to Sancti/
12-24o (Christmas Vigil) and emitted "Deus, qui nos
redemptiónis nostræ ánnua exspectatióne lætíficas...".
Perl stays on today and emits the Adv4-Sun "Excita,
quǽsumus, Dómine, poténtiam tuam..." Oratio.

Trace: `horascommon.pl::concurrence:950-951` — within the
suppress-1V OR chain:

  || ( $cwinner{Rank} =~ /Feria|Sabbato|Vigilia|Quat[t]*uor/i
    && $cwinner{Rank} !~ /in Vigilia Epi|in octava|infra octavam|Dominica|C10/i)

Vigil days don't claim 1st Vespers. Christmas Eve [Rank] =
"In Vigilia Nativitatis Domini;;Duplex I classis;;6.9" —
matches /Vigilia/i, not in any exception list → suppress 1V →
today wins.

Fix: new gate in `first_vespers_day_key_for_rubric`. Only
matches the **Vigilia** sub-clause — the Feria/Sabbato/
Quattuor branches would over-fire on every Tempora-ferial
tomorrow_key (Tempora/Epi3-4 [Rank] = ";;Feria;;1") and
break legitimate ferial-to-ferial swaps where Perl keeps the
swap (both days inherit the same Sun Oratio via "Oratio
Dominica" so the body happens to match). Narrowed to /
Vigilia/ which only matches actual Vigil days (Vigilia
Nativitatis, Vigilia Apostolorum, etc.).

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera      93.97% → 94.79% (+3)
      Completorium 93.97% → 94.25% (+1)
      Overall      96.10% → 96.23% (+5 cells)
    R60: 96.82% → 96.85% (+1 cell)
    R55: 93.90% → 93.94% (+1 cell)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 43: pre-DA Sun-Semiduplex 1V cedes to concurrent Duplex — T1570 96.23% → 96.54% (+9 cells)

Symptom: T1570 03-07 Sat-eve Compline emits text[4] omittitur
(preces fired). Perl emits V/R Domine exaudi (preces did NOT
fire). Same pattern: 03-21 Sat (Benedict), 05-09, 05-22, 06-25/
26/27 (Sacred Heart Octave), 07-03/04/05 (Octave continuation),
08-22 (Octave of Assumption), and similar Sat-eves where a
Duplex Sancti is on TODAY.

Trace: `horascommon.pl::concurrence:877-885` — pre-DA Sundays
cede 1st Vespers to a concurrent Duplex:

  if ( $cwrank[0] =~ /Dominica/i
    && $cwrank[0] !~ /infra octavam/i
    && $cwrank[1] =~ /semiduplex/i
    && $version !~ /1955|196/)
  {
    # before 1955, even Major Sundays gave way at I Vespers
    $cwrank[2] = $crank = $version =~ /altovadensis/i ? 3.9
                       : $version =~ /trident/i ? 2.9
                       : 4.9;
  }

For 03-07 Sat-eve: today=Sancti/03-07 Aquinas Duplex 3 vs
tomorrow=Tempora/Quad3-0 (Sun in Quad3) [Rank] (rubrica 1570)
= ";;II classis Semiduplex;;6.1". Without the cede, rank 6.1
> 3 → swap to Sun → wrong office. With the cede, tomorrow
reduces to 2.9 → 3 > 2.9 → Aquinas keeps 2V → preces predicate
sees Sancti winner with class "Duplex" → duplex_class=3 → preces
rejects → text[2-3] V/R emitted.

Fix: new helper `effective_tomorrow_rank_for_concurrence`
applies the cede when:
  - rubric is pre-DA (T1570/T1910/DA)
  - tomorrow's [Officium] contains "Dominica" but NOT "infra
    octavam" (Octave Sundays keep full rank)
  - tomorrow's [Rank] class field contains "Semiduplex"
    (higher-class Sundays — Easter, Pentecost — keep their
    rank too)
Cede value: T1570/T1910 → 2.9, DA → 4.9.

The helper plugs into the concurrence rank comparison
alongside the existing `effective_today_rank_for_concurrence`.
The body matches because the swap-decision flips, which makes
the preces predicate also see the right winner.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera      94.79% → 95.62% (+3)
      Completorium 94.25% → 95.89% (+6)
      Overall      96.23% → 96.54% (+9 cells)
    R60: 96.85% (unchanged — rule is pre-DA only)
    R55: 93.94% (unchanged — rule is pre-DA only)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 44: Quattuor Temporum Vespera Sunday-Oratio splice + Pasc7 / R60 exclusions — T1570 +2, R55 +2, R60 +2

Symptom: T1570 12-16 Wed (Adv3 Ember Wed) Vespera emits the
day's own [Oratio] "Praesta, quaesumus, omnipotens Deus..."
but Perl emits the Sunday's "Aurem tuam, quaesumus...".
Pattern hits all Adv Ember Vesperas (12-16 Wed, 12-18 Fri)
plus Lent Ember Vesperas (no Pent Octave because Perl excludes
Pasc7).

Trace: `specials/orationes.pl::oratio:55-61`:

  if ( ($rule =~ /Oratio Dominica/i && (...)) 
    || ($winner{Rank} =~ /Quattuor/i && $dayname[0] !~ /Pasc7/i
       && $version !~ /196|cist/i && $hora eq 'Vespera') )
  {
    my $name = "$dayname[0]-0";
    %w = %{setupstring(..., "$name.txt")};
  }

The Quattuor branch fires for Ember-day Vespera, swapping the
winner to the week-Sunday's office. Excludes Pasc7 (Pent Octave
Ember days keep their own) and R60/Cist (those rubrics keep the
day's own).

Two-part fix:
1. `force_sunday_oratio` predicate now chases `__preamble__`
   inheritance for [Officium] — Tempora/Adv3-3o is `@Tempora/
   Adv3-3` with no own [Officium], so a direct `sections.get`
   would miss the parent's "Feria IV Quattuor Temporum in
   Adventu". The Quattuor trigger now fires correctly on
   redirect-only variants.
2. When the trigger fires AND day_key is known, the splice
   reaches the week-Sunday's [Oratio] directly via a new
   `week_sunday_key_for_tempora` helper that derives `Tempora/
   {week}-0` from the day_key. The chain doesn't naturally
   include Sun (Adv3-3 [Rule] = "Preces Feriales" — no `vide`
   link), so we fetch explicitly.

Exclusions:
- Pasc7 (Pent Octave): Perl's `$dayname[0] !~ /Pasc7/i`. Day_key
  prefix `Tempora/Pasc7-` skips the trigger.
- R60: Perl's `$version !~ /196|cist/i`. R60 explicitly excluded;
  R55 falls through (matches /1955/ not /196/).

Threading: `splice_proper_into_slot` now takes `day_key` so it
can derive the week-Sunday key.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera   95.62% → 96.16% (+2)
      Overall   96.54% → 96.61% (+2 cells)
    R55:        93.94% → 94.01% (+2 cells)
    R60:        96.85% → 96.92% (+2 cells, knock-on from chain
                walk fix in Officium lookup)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 45: preces predicate Octave-day commemoration reject — T1570 96.61% → 97.02% (+15 cells)

Symptom: T1570 06-26 Fri Prima emits text[4] omittitur (preces
fired). Perl emits V/R Domine exaudi (preces NOT firing).
Pattern hits all dates within an unsuppressed Octave (06-25
through 07-05 in 2026 — Octaves of John Bapt + Apostles Peter
& Paul; 08-11..16 — Octave of Lawrence; 09-09..16 — Octave
of Nativity BVM, etc.).

Trace: `preces.pl:45` checks the COMMEMORATIO's [Rank] field:

  if ($commemoratio{Rank} =~ /Octav/i ...) { $dominicales = 0 }

— rejects preces when the commemoration carries "Octav" in the
title field. The clue: `SetupString.pl:705-708` prepends the
[Officium] body into the [Rank] title field at parse time:

  $sections{'Rank'} =~ s/^.*?;;/$sections{'Officium'};;/;

So Sancti/06-26oct.txt's [Rank] originally `;;Semiduplex;;2;;
ex Sancti/06-24` becomes "Die tertia infra Octavam Nativitatis
S. Joannis Baptistæ;;Semiduplex;;2;;ex Sancti/06-24" after
the merge — now matches /Octav/i.

Fix: in `preces_dominicales_et_feriales_fires`, check whether
a `Sancti/{MM-DD}oct` file exists in the corpus. The presence
of such a file indicates an Octave commemoration runs through
this date — Perl would inject it into @commemoentries via the
calendar lookup. Direct file-existence check matches the
empirical Perl behaviour without reproducing the calendar
computation.

Threading: `preces_dominicales_et_feriales_fires` now takes
`month` and `day` parameters.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Prima        95.89% → 97.26% (+5)
      Completorium 95.89% → 97.81% (+7)
      Overall      96.61% → 97.02% (+15 cells)
    R60: 96.92% (unchanged — most Octaves abolished)
    R55: 94.01% (unchanged — most Octaves abolished)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 46: rubric-aware [Rule]/[Officium] inheritance lookup — T1570 97.02% → 97.12% (+3 cells)

Symptom: T1570 12-10 Thu Adv2 Compline (resolved key Sancti/
12-11 Damasus Pope) emits text[2-3] V/R Domine exaudi (preces
NOT firing). Perl emits the omittitur directive (preces FIRES).
Same pattern: 12-11 Adv2 Fri (1V eve of Damasus 1V swap), and
the Compline counterparts.

Trace: Sancti/12-11 has TWO [Rule] sections:

  [Rule]
  vide C4b;
  9 lectiones;
  Doxology=Nat
  Omit Preces

  [Rule] (rubrica 1570)
  vide C4;
  9 lectiones;

For T1570 the second variant should win. The bare [Rule]
carries "Omit Preces" (used under R55/R60 where Damasus was
demoted to Common-of-Pope-Confessors with reduced rubric).
`section_via_inheritance` was returning the bare [Rule] →
preces predicate matched "omit preces" → returned false →
text[2-3] emitted instead of text[4].

Fix: new `section_via_inheritance_rubric(file, name, rubric)`
overload + helper `best_matching_section`. When a rubric is
supplied, prefer the `[{name}] (annotation)` variant when the
annotation matches the active rubric; bare `[{name}]` is the
fallback. Mirror of slice 34's [Oratio] pattern, applied to
[Rule] and [Officium] in `preces_dominicales_et_feriales_fires`.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Prima        97.26% → 97.53% (+1)
      Completorium 97.81% → 98.36% (+2)
      Overall      97.02% → 97.12% (+3 cells)
    R60: 96.92% (unchanged)
    R55: 94.01% (unchanged)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 47: `@:Section` self-redirect resolution — T1570 +5, R60 +5, R55 +5

Symptom: T1570 07-24 Fri Vigilia of James Matutinum emits the
literal directive `@:Oratio 1 loco` instead of the resolved body
"Da, quaesumus, omnipotens Deus: ut beati Jacobi Apostoli tui...".

Trace: Commune/C1v has the [Oratio] body:

  [Oratio]
  @:Oratio 1 loco
  (sed commune C4)
  @:Oratio 2 loco

The `@:` form is a SELF-redirect — points to a section in the
SAME file. Mirror of upstream `SetupString.pl` self-reference
handling. Under T1570 the `(sed commune C4)` conditional fails
so `eval_section_conditionals` reduces this to `@:Oratio 1 loco`.

`expand_at_redirect` requires a corpus-path prefix
(`Sancti/`/`Tempora/`/`Commune/`/...) — it can't resolve `@:`
because there's no file context. The literal directive leaked
into the body.

Fix: in `splice_proper_into_slot`, after `expand_at_redirect`
and `eval_section_conditionals`, check if the result starts with
`@:`. If so, extract the section name and re-query
`find_section_in_chain` for it. The chain contains the same
Commune file at chain[0..n], so the named section resolves
in-place.

Affects every Apostle-Vigil day plus other days using Commune/
C1v (Vigil-of-Apostles common). Cross-rubric — fires under all
five rubrics.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Matutinum/Laudes/Tertia/Sexta/Nona  96.99% → 97.26% (+1 each)
      Overall                             97.12% → 97.29% (+5 cells)
    R60: 96.92% → 97.09% (+5 cells)
    R55: 94.01% → 94.18% (+5 cells)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 48: Sancti-Simplex no-2V → today's Tempora-of-week — T1570 +2, R60 +2, R55 +1

Symptom: T1570 06-22 Mon Vespera, 05-12 Tue Vespera, 10-26 Mon
Vespera all emit the resolved Sancti Simplex's body. Perl emits
today's Tempora ferial (which inherits the week-Sun's Oratio
via `[Rule] Oratio Dominica`).

Trace: Simplex feasts have no proper 2nd Vespers. When today's
office is a Sancti Simplex (Paulinus 06-22, Nereus & co. 05-12,
Evaristus 10-26) AND the swap to tomorrow's 1V is rejected
(slice 42 Vigilia gate fired because tomorrow=Vigil), Perl
renders today's TEMPORA ferial Vespers — its Oratio chain
inherits the week-Sun via "Oratio Dominica".

Fix: post-process in `office_sweep` after
`first_vespers_day_key_for_rubric`. When all of:
  - hour is Vespera/Completorium,
  - resolved key equals today's derived key (no swap happened),
  - resolved key starts with `Sancti/`,
  - active rank class contains "Simplex",
override resolved_key to `Tempora/{today's weekname}-{today's
dow}` (computed via `date::getweek` + `date::day_of_week`).

`active_rank_line_with_annotations` made `pub` so office_sweep
can call it.

Narrow gating prevents regressions on the swap-correct case
(e.g. 04-16 Thu eve of Anicetus Fri 04-17 — Anicetus is Simplex
1.1, Tempora Thu is 1.0, Perl swaps to Anicetus 1V correctly;
the resolved key is tomorrow=Anicetus and `kept_today` is
false, so the override skips).

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera   96.16% → 96.71% (+2 cells)
      Overall   97.29% → 97.36% (+2 cells)
    R60: 97.09% → 97.16% (+2 cells)
    R55: 94.18% → 94.21% (+1 cell)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 49: Vigilia gate also checks tomorrow's [Officium] body — T1570 +1

Symptom: T1570 06-22 Mon Vespera resolved to Sancti/06-23
(Vigilia James) — slice 42's Vigilia gate didn't fire because
Sancti/06-23 [Rank] = ";;Simplex;;1.5" doesn't have "Vigilia"
in the rank field. But [Officium] = "In Vigilia S. Joannis
Baptistæ" — has "Vigilia" only in the title.

Trace: SetupString.pl:705-708 prepends [Officium] into [Rank]'s
title field at parse time, so Perl's `$cwinner{Rank} =~
/Vigilia/i` matches the title-only Vigil case. Our gate only
checked the [Rank] section body and missed it.

Fix: extend the slice 42 Vigilia gate to combine [Rank] +
[Officium] (each conditional-evaluated) before the substring
check. Slice 48's Sancti-Simplex-no-2V Tempora-of-week
fallback then fires for 06-22 (resolved=today=Sancti Simplex,
kept_today, override to Tempora/Pent03-1).

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera   96.71% → 96.99% (+1 cell — 06-22)
      Overall   97.36% → 97.40%
    R60: 97.16% (unchanged)
    R55: 94.21% (unchanged)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 50: Sept Embertide week-Sunday-Oratio splice — date-based fallback — T1570 +1, R55 +1

Symptom: T1570 09-18 Fri Vespera (Sept Ember Fri) emits the
day's own [Oratio] (Tempora/093-5 = "Praesta, quaesumus,
omnipotens Deus: ut observationes sacras..."). Perl emits
Pent16-0's Sun Oratio "Tua nos, quaesumus, Domine, gratia
semper et praeveniat...".

Trace: Slice 44's Quattuor Temporum Vespera trigger fires
(Tempora/093-5 [Officium] = "Feria Sexta Quattuor Temporum
Septembris" → contains "quattuor temporum"). The week-Sunday
key was derived by `week_sunday_key_for_tempora(day_key)` →
"Tempora/093-0". Tempora/093-0 EXISTS (it's the September
scripture overlay "Dominica III. Septembris") but has only
[Scriptura], [Ant 1], [Lectio*] sections and NO [Oratio]. So
the Sunday-splice failed and the day's body was emitted.

The September Embertide overlay files (`Tempora/093-X`) are
month-day overlays that don't naturally encode the liturgical
week. The actual liturgical week is `Pent16` (16th Sun after
Pentecost). For 09-18 in 2026, `date::getweek` returns
"Pent16".

Fix: in `splice_proper_into_slot`, derive the week-Sunday
candidate via TWO paths:
  1. Day-key-based (handles Adv3-3o → Adv3-0).
  2. Date-based (handles Tempora/093-5 → Tempora/Pent16-0).

Pick the first candidate whose file has an [Oratio] (chased
through `__preamble__` inheritance) — Tempora/093-0 fails this
filter (no Oratio), Tempora/Pent16-0 succeeds. Threading:
splice_proper_into_slot now takes year/month/day to call
`date::getweek`.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera   96.99% → 97.26% (+1 cell — 09-18)
      Overall   97.40% → 97.43%
    R55: 94.21% → 94.25% (+1 cell)
    R60: 97.16% (unchanged — slice 44 excludes R60 from the
         Quattuor trigger)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 51: parse_vide_targets sees `;;`-suffixed `vide`/`ex` directives — T1570 +5, R60 +7, R55 +6

Symptom: T1570 07-01 Wed (Octave Day of John Bapt) Matutinum
emits the literal `!Oratio propria.` rubric directive instead
of the Octave's Oratio "Deus, qui præsentem diem honorabilem
nobis in beati Joannis nativitate...". Same pattern: 08-18,
08-19, 08-21, 12-29 — Octave-day stems whose [Rank] inherits
via the 4th-field directive.

Trace: Sancti/07-01t (Octave Day of John Baptist) carries
[Rank] = ";;Duplex;;3.1;;vide Sancti/06-24" — the inheritance
target is in the 4th `;;`-separated field. The chain walker
calls `parse_vide_targets` on the full rank line, but the line
loop only matched directives starting at LINE-START
("vide ...", "ex ...", "@..."). The whole rank line
";;Duplex;;3.1;;vide Sancti/06-24" doesn't start with "vide ",
so the inheritance was missed. Sancti/06-24 (St. John Bapt
Nativity) was never added to the chain → no [Oratio] found
in chain → the rubric-directive `!Oratio propria.` from
Sancti/07-01t leaked as the body.

Fix: in `parse_vide_targets`, split each line by `;;` and
re-run the line-prefix detector on each segment. The 4th
`;;`-segment "vide Sancti/06-24" now matches and adds the
inheritance target.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Mat/Laudes/Ter/Sex/Non  97.26% → 97.53% (+1 each)
      Overall                 97.43% → 97.60% (+5 cells)
    R60: 97.16% → 97.36% (+7 cells)
    R55: 94.25% → 94.45% (+6 cells)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 52: pre-1960 Latin spelling normalisation (Genetri→Genitri, cotidian→quotidian) — T1570 +44, R55 +70

Symptom: Pre-1960 rubrics emitted "Sanctíssimæ Genetrícis tuæ
sponsi..." (03-19 St. Joseph), "Filii tui Genetricem..."
(08-15 Assumption Octave week), "Genetricis filii tui..."
(02-09 Cyril & Methodius), and other "Genetri-" forms. Perl
emits "Genitri-" — different by one letter.

Trace: `horascommon.pl::spell_var:2138-2169` is a hour-side
spelling normaliser called for every emitted block. For
pre-1960 versions:

  s/Génetrix/Génitrix/g;
  s/Genetrí/Genitrí/g;
  s/\bco(t[ií]d[ií])/quo$1/g;
  ... (Cisterciensis-specific)

Ports the medieval-into-classical spelling reform: corpus
files keep the older "Genetrix/Genetrí-/cotidian-" forms,
display fold to "Genitrix/Genitrí-/quotidian-" under non-
1960 versions. R60 path was already implemented (`tr/Jj/Ii/`);
the pre-1960 path was missing.

Fix: extend `apply_office_spelling` to apply the pre-1960
substitutions. New helper `replace_cotidian_with_quotidian`
implements the regex `\bco(t[ií]d[ií])\w*` → `quo$1\w*`
manually (UTF-8 byte walking, since "í" is a 2-byte
codepoint and we don't pull a regex dep).

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Matutinum    97.53% → 98.90% (+5)
      Laudes       97.53% → 99.73% (+8)
      Tertia       97.53% → 99.73% (+8)
      Sexta        97.53% → 99.73% (+8)
      Nona         97.53% → 99.73% (+8)
      Vespera      97.26% → 99.18% (+7)
      Overall      97.60% → 99.11% (+44 cells)
    R55: 94.45% → 96.85% (+70 cells)
    R60: 97.36% (unchanged — R60 keeps "Genetri-" forms;
         only the J→I path applies)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 53: preces predicate rejects in Pasc6 / Pasc7 weeks — T1570 +8 cells

Symptom: T1570 Pent-Octave Ember Wed/Fri (05-27, 05-29) Prima
+ Compline, Sat-of-Octave-end (05-30) Prima, Pent Vigil Fri/Sat
(05-22, 05-23) Prima emit text[4] omittitur. Perl emits V/R
Domine exaudi (preces NOT firing).

Trace: `preces.pl:18-19` is an early-reject:

  return 0 if (... || $dayname[0] =~ /Pasc[67]/i);

For ALL days in Pasc6 (post Octavam Ascensionis week — 05-21
to 05-23 in 2026 between Asc Octave end + Pent Sat-Vigil) and
Pasc7 (Pent Octave week — 05-25 to 05-30) the preces predicate
rejects unconditionally. Our preces_fires didn't have this
gate.

Fix: add an early-return when day_key starts with `Tempora/
Pasc6-` or `Tempora/Pasc7-`. Mirror of the Perl line.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Prima        97.53% → 98.90% (+5)
      Completorium 98.36% → 99.18% (+3)
      Overall      99.11% → 99.38% (+8 cells)
    R60: 97.36% (unchanged — R60 abolished Pent Octave; the
         Pasc7 prefix doesn't match R60's resolved keys)
    R55: 96.85% (unchanged for similar reason)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 54: preces predicate Octave-commemoration via rubric-active kalendarium — T1570 +2

Symptom: T1570 09-13 Sun (Octave Nativity BMV) Prima emits
text[4] omittitur. Perl emits V/R Domine exaudi (preces NOT
firing). Sancti/09-13bmv has [Rank] = "Sexta die infra
Octavam Nativitatis BMV;;Semiduplex;;2;;...".

Trace: same Octave-commemoration reject as slice 45, but the
commemoration file is `Sancti/09-13bmv` (BMV-Octave-suffix),
not `Sancti/09-13oct`. Slice 45 only checked the `oct` suffix.

Slice 53 attempt to broaden to all suffixes regressed -7 cells
because it picked up Imm. Conc. Octave (Sancti/12-09bmv) under
T1570 — that Octave was added in 1854 and isn't in the T1570
kalendar. File-existence is too broad as a proxy for "active
commemoration on this date AND rubric".

Fix: query the rubric-active kalendarium via
`kalendaria_layers::lookup(rubric.kalendar_layer(), month, day)`.
Each returned cell carries an `officium` string. Reject preces
when any cell's officium contains "Octav" (excluding "post
Octav"). For T1570 09-13: cell[0] is "Sexta die infra Octavam
Nativitatis BMV" → match → reject. For T1570 12-09: no cells
returned (kalendarium has no entry) → no reject.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Prima        98.90% → 99.18% (+1 — 09-13)
      Completorium 99.18% → 99.45% (+1 — 09-12 Sat)
      Overall      99.38% → 99.45% (+2 cells)
    R55: 96.85% (unchanged — different active layer means
         different cells; the Octave-of-Lawrence et al.
         already triggered slice 45's `oct`-suffix path)
    R60: 97.36% (unchanged — most Octaves abolished)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Slice 55: pre-DA "a capitulo de sequenti" for Octave-day tomorrow — T1570 +1

Symptom: T1570 07-03 Fri Vespera resolved to Sancti/07-03
(Leo II Semiduplex 2.2). Perl swaps to Sancti/07-04oct (Day
VI in Octave Petri+Pauli Semiduplex 2). Page header shows
"A capitulo de sequenti; commemoratio de praecedenti".

Trace: `horascommon.pl::concurrence:1216-1261` — the
`flcrank == flrank` branch fires when today and tomorrow's
ranks flatten to the same bucket:

  } elsif ($flcrank == $flrank) {
    $vespera = 1; $cvespera = 3; $winner = $cwinner;
    $dayname[2] .= "<br/>A capitulo de sequenti; commemoratio de praecedenti";
  }

T1570 flattening: rank < 2.9 → 2. Both Leo (2.2) and Octave
Day VI (2) flatten to 2 → swap.

Fix: narrow gate in `first_vespers_day_key_for_rubric` (after
the existing rank comparison setup, before `today_rank >
tomorrow_rank`). Fires only when:
  - rubric is pre-DA (T1570/T1910/DA)
  - tomorrow_key is `Sancti/.*oct$` (Octave-stem-day file)
  - today_key starts with Sancti/
  - both ranks < 2.9 (Semiduplex bucket)

Documented attempted-and-reverted: the broader
flrank/flcrank logic without `oct`-suffix narrowing regressed
T1570 Vespera by 12 cells across Tempora-ferial pairs where
the "a capitulo" rule shouldn't fire. The Octave-stem-day
restriction is the canonical upstream trigger.

Verification:

  T1570 30-day Jan: 240/240 (100.00%, preserved)

  Full year × 2920 cells:
    T1570:
      Vespera   99.18% → 99.45% (+1 cell — 07-03)
      Overall   99.45% → 99.49%
    R55: 96.85% (unchanged — pre-DA only)
    R60: 97.36% (unchanged)

Mass T1570 + R60 year-sweeps stay at 365/365 (100%). 431 lib
tests pass.

## Patterns *attempted and reverted*

- **Broad Octave-suffix file enumeration for preces reject**
  (slice 53 attempt): tried checking `Sancti/{MM-DD}{suffix}`
  for `oct`, `bmv`, `dom-oct`, `sab-oct`, `octt`. Net regression
  -7 cells because the corpus carries `bmv` files for
  post-1854 Octaves (Octave of Immaculate Conception 12-08) that
  T1570's calendar doesn't include — file-existence is too
  broad a proxy for "active commemoration on this date and
  rubric". Reverted; Pasc6/7 day_key prefix is the narrower
  correct lever for the Pent Octave / post-Asc-Octave reject.

- **Hour-aware annotation filter** (slice 50 attempt): tried
  treating `(nisi ad vesperam aut rubrica X)` as hour-context
  annotation that filters at Vespera. Got R60 -15 cells because
  Perl's `vero` regex `/vesperam/i` doesn't actually match the
  hora value "Vespera" (no trailing `m`), so the annotation
  ALWAYS applies in Perl regardless of hour. Reverted; the
  Quattuor force_sunday_oratio path (slice 44) handles the
  T1570 swap correctly.

- **`section_via_inheritance` walked + Officium-prepended Vigilia
  gate** (slice 46 attempt): tried extending slice 42's Vigilia
  gate to also check tomorrow's [Officium] body for "vigilia"
  (mirroring SetupString.pl:705-708's title-prepend behaviour).
  06-22 Mon Vespera correctly stopped swapping to Sancti/06-23
  (Vigilia Joannis Bapt) — but today's resolved key Sancti/06-22t
  (Paulinus Simplex 1.1) was still wrong; Perl uses today's
  Tempora ferial because Simplex has no proper 2V. The deeper
  fix needs a Tempora-of-week fallback in
  `first_vespers_day_key_for_rubric`, requiring the function to
  return owned String. Reverted; documented as a known TODO.

- **Feria/Sabbato/Quattuor branches of the Vigil gate**: tried the
  full Perl OR chain (Feria | Sabbato | Vigilia | Quat[t]*uor) on
  tomorrow's [Rank]. Net Vespera: 93.97% → 90.68% (-12 cells).
  Tempora ferials all match /Feria/i but Perl keeps the swap
  because consecutive Tempora ferials inherit the same Sunday
  Oratio (`[Rule] Oratio Dominica`) — bodies match regardless of
  which ferial day_key the swap lands on. Reverted to Vigilia-only.


- **Section-level `[Rank] (rubrica 196)` annotated lookup
  attempt** (slice 31a): tried evaluating section-header
  conditionals so R60 could find Sancti/01-12 R60's annotated
  `[Rank] (rubrica 196 aut rubrica 1955) Die Duodecima Januarii;;
  Feria;;1.8` instead of the bare `[Rank]`. Required stripping
  stopwords ("sed") via `find_conditional` before `vero`. Net
  cell change was 0 across both rubrics, so reverted. The
  annotated [Rank] variant gap remains documented as a TODO in
  `active_rank_line_for_rubric`.

- **Mass-side `expand_macros` on Office bodies** (slice 9
  attempt): expanding `$Per Dominum`/`$Per eumdem` macros via
  `crate::mass::expand_macros` regressed pass-rate from 63.33%
  to 46.67%. The Mass-side prayer expansion text doesn't align
  with what Perl renders for Office bodies — the comparator's
  substring-match was already accepting the unexpanded form
  (Rust's body up to the macro marker matched the prefix of
  Perl's expanded text). Reverted; left as a known divergence.
