# Mass-side regression residuals (1976–2076 × 5 rubrics)

**Generated:** 2026-05-06 against the persistent-driver-corrected sweep
(post-`c093c2f`). Re-generate via:

```bash
./target/release/year-sweep --years 1976:2076 --rubric "<RUBRIC>" --quiet
python3 scripts/aggregate_sweep.py --years 1976:2076 --top 25
```

## Summary table

| Rubric          | Pass-rate              | Fail-days | At goal (100%)? |
|-----------------|------------------------|-----------|-----------------|
| Tridentine 1570 | **100.00%** (36891/36891) | **0**     | ✅ |
| Tridentine 1910 |  99.74% (36796/36891) | 95         | ❌ |
| Divino Afflatu  |  99.85% (36836/36891) | 55         | ❌ |
| Reduced 1955    |  99.66% (36764/36891) | 127        | ❌ |
| Rubrics 1960    |  99.50% (36705/36891) | 186        | ❌ |
| **total**       |                          | **463**    |   |

T1570 hit 100% on the first run after the persistent-driver `setupstring`
cache fix. All four post-1570 rubrics have residuals — these are the
real Phase 7–10 reform-layer gaps, not harness artifacts.

## Root-cause clusters (ordered by leverage)

Each cluster is a single logical bug whose fix accounts for the listed
fail-day count.

### 1. Rogation Days propers chain (R60) — **124 days**

Pattern (top winner-pair):

```
34× Tempora/Pasc6-2 -> Tempora/Pasc5-4
33× Tempora/Pasc6-3 -> Tempora/Pasc5-4
32× Tempora/Pasc6-1 -> Tempora/Pasc5-4
25× Tempora/Pasc5-5 -> Tempora/Pasc5-4
```

Inferred Perl-source (Rust-missed): `Tempora/Pasc5-4:Evangelium 124×`.

Pasc5-4 = Wednesday in week 5 of Easter = the Greater Litany / Mark's
Day Mass (April 25 transferred). Under R1960, the three Rogation
Days (Mon/Tue/Wed before Ascension = Pasc6-1/2/3) AND the Thursday
following the Mark Mass (Pasc5-5) reuse the Pasc5-4 readings
chain. Rust currently picks each day's own (mostly empty) winner
file; Perl follows a redirect chain.

* **Fix scope:** R1960-only chain rule. Add a precedence-time redirect
  for `Pasc5-5`/`Pasc6-1`/`Pasc6-2`/`Pasc6-3` → `Pasc5-4` body when
  `rubric == Rubrics1960`.
* **Phase:** 10.

### 2. Octave-of-Epiphany Sunday displaced (DA) — **135 days**

Pattern:

```
75× Tempora/Epi1-0 -> Sancti/01-06
60× Tempora/Epi1-0 -> Sancti/01-13
```

Inferred miss: `Sancti/01-06:*` (Epiphany octave-day) +
`Sancti/01-13:*` (Baptism of the Lord) sections.

Under Divino Afflatu (1939+), the Sunday-after-Epiphany is displaced
by the Holy Family / Baptism of the Lord — not the Tempora/Epi1-0
"Sunday within the Octave of Epiphany" body that Rust picks. Two
distinct displacements depending on which Sunday falls on which
date (the 75/60 split).

* **Fix scope:** DA precedence layer — Sunday-within-Octave-of-Epiphany
  yields to fixed Sancti feasts.
* **Phase:** 8.

### 3. Trinity-Friday → Trinity-Sunday body (R55) — **75 days**

Pattern: `Tempora/Pent01-5 -> Tempora/Pent01-0`.

Inferred miss: `Tempora/Pent01-0:Oratio` + `:Secreta` + `:Postcommunio`.

Friday after Trinity Sunday under R1955 reads the Trinity Sunday
collect/secret/postcommunion bodies. Pent01-5 itself is a feria
under the reform.

* **Fix scope:** R1955 precedence — Pent01-5 yields to Pent01-0 body.
* **Phase:** 9.

### 4. September Ember days / Exaltation of Cross (DA + R55) — **84 days**

Pattern:

```
24× Tempora/Pent16-0 -> Sancti/09-14
24× Tempora/Pent17-0 -> Sancti/09-14
18× Tempora/Pent14-0 -> Sancti/09-14
12× Tempora/Pent15-0 -> Sancti/09-14
 6× Tempora/Pent18-0 -> Sancti/09-14
```

When 09-14 (Exaltation of the Holy Cross) falls on a Sunday in
weeks 14–18 after Pentecost, the Sancti feast wins the Sunday and
the Pent body is suppressed under post-1570 rubrics.

* **Fix scope:** DA + R55 precedence — Sancti/09-14 outranks ferial
  Pent ordinary-time Sundays.
* **Phase:** 8 (DA), 9 (R55) — same rule, both layers.

### 5. T1910 Sancti/02-22 / 02-23r / 02-24 cluster — **~70 days**

Pattern:

```
38× Sancti/02-23r -> Sancti/02-24
32× Sancti/02-22  -> Sancti/02-24
```

Cathedra Petri Romae (02-22, post-1908 reformation) +
Vigil-of-Matthias (02-23r) → Sancti/02-24 (St. Matthias). Bissextile
shifts complicate it.

* **Fix scope:** T1910 sancti precedence + transfer-table port.
* **Phase:** 7.

### 6. T1910 Sancti/03-25 Annunciation in Lent — **~40 days**

Pattern: `Sancti/03-25 -> Tempora/Quad4-0`.

When Annunciation (03-25) falls on a Lent Sunday, T1910 gives the
day to the Sunday with Annunciation as commemoration; Rust picks
the saint as the winner.

* **Fix scope:** T1910 Lent-vs-Annunciation precedence rule.
* **Phase:** 7.

### 7. Self-references on Pent19–23 Sundays (R55 + R60) — **~80 days**

Pattern:

```
21× Tempora/Pent20-0 -> Tempora/Pent20-0   (R60)
18× Tempora/Pent21-0 -> Tempora/Pent21-0   (R60)
15× Tempora/Pent19-0 -> Tempora/Pent19-0   (R60)
17× Tempora/Pent20-0 -> Tempora/Pent20-0   (R55)
12× Tempora/Pent19-0 -> Tempora/Pent19-0   (R55)
12× Tempora/Pent22-0 -> Tempora/Pent22-0   (R55)
12× Tempora/Pent21-0 -> Tempora/Pent21-0   (R55)
```

Same file → same file means Rust picked the right WINNER but the
SECTION CONTENT diverges. Likely a `[Section] (rubrica 1960)` /
`(rubricis 1955)` variant evaluator gap — Rust pulls the default
section, Perl honors a per-rubric override on the same file.

* **Fix scope:** check `rubric_variant_section_for` against Pent19/20/21/22/23-0
  files; broaden if the variant marker is something we don't yet match.
* **Phase:** 9 (R55) + 10 (R60).

### 8. Quadp / Quad ferias → Commune/C4a (T1910 + DA + R55) — **~50 days**

Pattern (combined):

```
30× Tempora/Quadp2-1 -> Commune/C4a   (T1910)
24× Tempora/Quad1-1  -> Commune/C4a   (T1910)
24× Tempora/Quadp1-2 -> Commune/C2-1  (T1910)
24× Tempora/Quad2-1  -> Commune/C4a   (T1910)
12× Tempora/Quadp2-1 -> Commune/C4a   (DA + R55)
```

Septuagesima / Quadragesima Mondays — when a Sancti commemoration
fires, the section content goes through a Commune redirect that
Rust isn't following on these specific days.

* **Fix scope:** Sancti-commemoration commune fallback for Quadp/Quad
  ferias. Probably one rule shared across T1910/DA/R55.
* **Phase:** 7+ (cross-rubric).

### 9. Pent22/Pent19 → Commune/Propaganda (R55) — **24 days**

Pattern:

```
12× Tempora/Pent22-0 -> Commune/Propaganda
12× Tempora/Pent19-0 -> Commune/Propaganda
 9× Tempora/Pent21-0 -> Commune/Propaganda
```

Diocese-conditional Mass (Propaganda Fide). Commune/Propaganda is
a special variant for missionary dioceses, conditioned on
`$dioecesis`. Rust likely doesn't honor the dioecesis-conditional
section.

* **Fix scope:** dioecesis-conditional commune evaluator.
* **Phase:** 9 — but probably tractable as a small standalone fix.

## Attack plan — leverage order

Sorted by fail-days unlocked per slice:

| Order | Cluster | Days unlocked | Phase | Risk |
|-------|---------|---------------|-------|------|
| 1 | DA Epiphany-Sunday displacement (#2) | 135 | 8 | medium — precedence rule |
| 2 | R60 Rogation Days chain (#1) | 124 | 10 | low — single redirect rule |
| 3 | September Ember/Cross (#4) | 84 | 8+9 | medium — same rule, two layers |
| 4 | Pent19–23 same-file variants (#7) | 80 | 9+10 | low — variant matcher |
| 5 | T1910 02-22/23r/02-24 (#5) | 70 | 7 | medium — transfer + precedence |
| 6 | R55 Trinity-Friday body (#3) | 75 | 9 | low — single rule |
| 7 | Quadp/Quad → C4a (#8) | 50 | 7+ | medium — cross-rubric |
| 8 | T1910 Annunciation (#6) | 40 | 7 | medium — Lent precedence |
| 9 | Propaganda commune (#9) | 24 | 9 | low — dioecesis conditional |

**No-stack-overflow caveat:** the Pent19–23 same-file variants
(#7) are an investigation slice — needs a 1-day diff dump first to
confirm the `(rubrica X)` story before being scheduled.

## Notes on the harness fix that unblocked this

The previous polluted run had 564 fail-days (T1570 alone showed 101).
After commit `c093c2f` (per-render `%setupstring_caches_by_version`
reset in the persistent driver), T1570 dropped to 0 fails. Net
correction: 101 fails were a driver bug, not a Rust bug.

For the post-1570 rubrics, the polluted-vs-corrected counts are
identical (95 / 55 / 127 / 186). Their residuals are real and
were already independent of the cache pollution.
