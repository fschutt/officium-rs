# Cluster-closure progress (autonomous loop)

**Last update:** 2026-05-07. Updated each iteration of the autonomous
loop (`/loop` dynamic, ~20 min cadence). The loop's exit criterion
is **all 11 clusters closed AND the final 100-year × 5-rubric sweep
returns 100% on every rubric.**

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

🎯 **Milestone: 100% across 6-year × 5-rubric sample (13146 days)** —
T1570, T1910, DA, R55, R60 all clean on 1979, 1985, 2000, 2025,
2050, 2074. Broader 15-year sweep (27410 days, includes leap years
1976/1996/2020/2032 + Easter-extreme 2032) shows 99.97% pass with
9 residuals isolated to: T1910 leap Feb-24/26/29 + 2032-04-06,
DA 1996-02-26, R60 10-18 Class-II-saint Sundays.

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
| 15 | DA_WMSunday_NonHilarion | 6× | ⛔ | DEFERRED Phase 9: DA on WMSunday is NOT sub-unica (separate Orémus per commemoration, parent's $Per kept). Differs from R55 sub-unica path. Implementation needs a different commemoration-emission shape than `apply_r55_simplex_commemoration`. |
| 16 | R55_WMSunday_NonHilarion | 3× | ⛔ | DEFERRED Phase 9: years where penultimate Sun ≠ 10-21 (e.g. 2000-10-22 = Pent22 + Cantius); under R55 Class III feasts on Sunday Mass-suppressed (Lauds-only). Need to verify Cantius is correctly suppressed under R55 too — currently fails. |
| 17 | R60_03_06_Perpetua_Felicitas | 3× | ✅ | closed by rubric-conditional `[Section] (rubrica 1960)` lookup |
| 18 | R60_WMSunday | 4× | ⏳ | R60 10-19/21/23 — WMSunday + Sancti commemoration |
| 19a | R60_Imm_Conc | 1× | ✅ | closed by RG 15 special-case in `decide_sanctoral_wins_1570` |
| 19b | R60_Joseph_0320 | 1× | ✅ | closed by rubric-aware `apply_transfer_sancti_1570` rank pick |
| 19c | R60_Christmas_Eve | 1× | ⏳ | 2000-12-24 Christmas Eve (Vigilia Nativitatis) — separate investigation |

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

- [ ] All 12 clusters closed in `target/regression/clusters/*.txt`.
- [ ] Final 100-yr × 5-rubric sweep returns 100% across the board.
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
