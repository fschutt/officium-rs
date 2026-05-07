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

## Patterns *attempted and reverted*

- **Mass-side `expand_macros` on Office bodies** (slice 9
  attempt): expanding `$Per Dominum`/`$Per eumdem` macros via
  `crate::mass::expand_macros` regressed pass-rate from 63.33%
  to 46.67%. The Mass-side prayer expansion text doesn't align
  with what Perl renders for Office bodies — the comparator's
  substring-match was already accepting the unexpanded form
  (Rust's body up to the macro marker matched the prefix of
  Perl's expanded text). Reverted; left as a known divergence.
