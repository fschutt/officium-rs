# Cluster-closure progress (autonomous loop)

**Last update:** 2026-05-07. Updated each iteration of the autonomous
loop (`/loop` dynamic, ~20 min cadence). The loop's exit criterion
is **all 11 clusters closed AND the final 100-year × 5-rubric sweep
returns 100% on every rubric.**

🏁 **EXIT CRITERION MET — full 1976:2076 × 5-rubric sweep:
184,455 / 184,455 days passing (100.00%).**

```
T1570  : 36891 / 36891  (100.00%)
T1910  : 36891 / 36891  (100.00%)
DA     : 36891 / 36891  (100.00%)
R55    : 36891 / 36891  (100.00%)
R60    : 36891 / 36891  (100.00%)
```

## Status table

| # | Cluster | Days | Closed | Commit |
|---|---------|------|--------|--------|
|  1 | R60_RogationDays            | 124 | ✅ | `10aefda` |
|  2 | DA_EpiphanySunday           |  15 | ✅ | `11be449` |
|  3 | Pent19_23_SelfRef_R60       |  28 | ✅ | `f040d62` |
|  4 | R55_Propaganda              |  11 | ✅ | `f040d62` |
|  5 | R55_TrinityFriday           |  24 | ✅ | `789cc19` |
|  6 | T1570_12_08o                |   0 | ✅ | `c093c2f` (driver-cache fix) |
|  7 | T1910_Cathedra_Matthias     |   9 | ✅ | leap-year transfer-table filter (Perl filter-1 port) |
|  8 | T1910_Annunciation          |   5 | ✅ | Adv/Quad srank cap: Class I sancti capped to 6.01 |
|  9 | DA_SeptEmbersCross          |  14 | ✅ | Festum Domini precedence in `decide_sanctoral_wins_1570` |
| 10 | R55_SeptEmbersCross         |  14 | ✅ | extended Festum Domini gate to include R55 |
| 11 | Pent19_23_SelfRef_R55       |  15 | ✅ | R55 simplex-commemoration appendage + Propaganda suppression gate |
| 12 | Quadp_Quad_Commune_C4a      |  29 | ✅ | T1910 sancti chain: main-stem-only suppression + dirge skip + layer-aware leap suppression + back-walk |

**Closed: 12 / 12 clusters, 463 / 463 fail-days (100%).** 🎯

🎯 **15-year × 5-rubric sample (27,410 days): 100% pass** —
T1570, T1910, DA, R55, R60 all clean on 1976, 1979, 1981, 1985,
1990, 1996, 2000, 2008, 2012, 2020, 2025, 2032, 2050, 2065, 2074.

🎯 **30-year × 5-rubric sample (54,790 days): 99.998% pass** —
single residual at T1910 2002-01-31. Petri Nolasci (file stem
01-28, kalendar 01-31 under 1906 layer) should be feria-suppressed
under f.txt + 331.txt rule `01-28=01-27~01-28t;;1570 M1617 1888 1906`,
but our `stem_transferred_away_with_stems` doesn't trigger. The
distinction between kalendar key and file stem (kalendar 01-28 has
Agnes stem 01-28t; kalendar 01-31 has Petri stem 01-28) requires
careful handling — naive source-stem match also breaks 1976 letter-c
year (where d.txt's `01-28=01-18` rule applies under filter-2 but
shouldn't suppress Petri at kalendar 01-31). Deferred — needs
deeper Perl `transfered()` trace.

🎯 **R60_03_06_Perpetua_Felicitas Communio closed** — chase to
`@Commune/C6-1` was looking up `Communio (rubrica 1960)` in the
chased file (which only has the bare `Communio` key). Fixed in
`chase_at_reference` by stripping a trailing `(annotation)` from
the `default_section` arg before lookup. The rubric-conditional
pickup at the winner level is unaffected — only the chase target
sees the bare name.

🎯 **T1910 letter-f easter-331 Petri-Nolasci suppression closed** —
narrow Perl `transfered()` substring path added to
`stem_transferred_away_with_stems`: when a rule keyed at our stem
mentions our stem as a substring of its val (typically a suffixed
sibling like `01-28t` literally containing `01-28`), suppress.
Gated on `source_mmdd == stem` so unrelated rules whose val happens
to mention our stem don't fire. Closes 1991, 2002, and any other
letter-f easter-331 year where rule
`01-28=01-27~01-28t;;1570 M1617 1888 1906` (Stransfer/331.txt)
fires Petri Nolasci feria-suppression on his native 01-31. 28-year
× 5-rubric wider sweep (140 pairs, ~51,000 days): 0 fails.

## Final exit gate

* All 12 ORIGINAL clusters closed (verified via `scripts/cluster_verify.sh`).
* T1570 returns 100% across all spot-checked years (1979, 1985,
  2000, 2025, 2050, 2074).
* T1910/DA/R55/R60 sample-year sweep surfaced **27 residual fails**
  not tracked by any of the 12 original clusters — listed below as
  follow-on clusters.

## New clusters (discovered post-12-cluster closure)

| # | Cluster | Days/yr | Status | Pattern |
|---|---------|---------|--------|---------|
| 13 | T1910_Septem_Fundatorum_0212 | 4× | ✅ | closed by T1910 heuristic over-fire guard |
| 14 | T1910_Joseph_0319_0320 | 2× | ✅ | closed by same |
| 17 | R60_03_06_Perpetua_Felicitas | 3× | ✅ | rubric-conditional `[Section] (rubrica 1960)` lookup + chase strips `(rubrica X)` annotation when chasing `@Commune/...` so the chased file's bare `Communio` is found |
| 18 | R60_WMSunday | 4× | ✅ | incidentally closed (verified by full sweep — 0 fails on every R60 day 1976-2076) |
| 19a | R60_Imm_Conc | 1× | ✅ | closed by RG 15 special-case in `decide_sanctoral_wins_1570` |
| 19b | R60_Joseph_0320 | 1× | ✅ | closed by rubric-aware `apply_transfer_sancti_1570` rank pick |
| 19c | R60_Christmas_Eve | 1× | ✅ | incidentally closed (verified by full sweep — 0 fails on every R60 day 1976-2076) |
| 15 | DA_WMSunday_NonHilarion | 6× | ✅ | incidentally closed (verified by full sweep — 0 fails on every DA day 1976-2076) |
| 16 | R55_WMSunday_NonHilarion | 3× | ✅ | incidentally closed (verified by full sweep — 0 fails on every R55 day 1976-2076) |

Sample-year sweep (1979, 1985, 2000, 2025, 2050, 2074):
* **T1570 100%** — 0 fails ✓
* **T1910 100%** — 0 fails ✓
* DA: 6 fails (cluster 15)
* R55: 3 fails (cluster 16)
* R60: 7 fails (clusters 18 + 19)

Total residuals: **16 fail-days across 6 sample years × 5 rubrics**
(of 13146 days; 99.88% pass).

## Closed-cluster regression note

Pent19_23_SelfRef_R55 (cluster 11) closure narrowed to World Mission
Sunday gate (`monthday key "104-0"`) after broader gate caused
duplicate-emission regressions on non-WMSunday R55 Sundays. See
`mass.rs::apply_r55_simplex_commemoration`.

## Iteration plan

Each loop iteration:

1. Pick the next ⏳ cluster with the smallest day-count (smallest
   blast radius first; bigger ones may need bigger ports).
2. Inspect a representative failing day via `year-sweep --dump`.
3. Locate the upstream Perl rule that makes the day pass.
4. Decide: narrow Rust-side fix (commit) OR Phase-7-10 port too big
   for one slice (mark cluster as deferred + document).
5. Run `cargo test` + `scripts/cluster_verify.sh CLUSTER`. Both
   must pass before commit.
6. Spot-check 2-3 already-closed clusters for regressions.
7. Update this doc; commit + push.

## Exit checklist

- [x] All 12 clusters closed in `target/regression/clusters/*.txt`.
- [x] All follow-on clusters (13-19c, 15-16) closed.
- [x] Final 100-yr × 5-rubric sweep returns 100% across the board.
- [ ] `docs/REGRESSION_RESIDUALS.md` updated to reflect closure.

## Open questions logged across iterations

* T1910 Sunday-vs-Sancti precedence: what's the upstream rule for
  Duplex II classis sancti vs Semiduplex Dominica II classis on Lent
  Sundays? (Annunciation, Cathedra Petri.) Likely
  `horascommon.pl::sancti_v_sundays` — investigate next slice.
* R55 commemoration-suppression by rank: under R55, Duplex (3) feasts
  on a Sunday don't commemorate (Cantius), but Simplex (1.1) feasts
  do (Hilarion). Counter-intuitive — needs Perl trace.
* Phase 8/9 reform-layer port scope: most remaining clusters need
  partial precedence-layer ports. Track effort against
  `docs/SUPER_PLAN.md` Phase 7-10 tasks.
