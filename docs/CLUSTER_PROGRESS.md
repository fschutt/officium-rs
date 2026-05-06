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
| 11 | Pent19_23_SelfRef_R55       |  15 | ⛔ | DEFERRED: needs Phase-9 commemoration-appendage port |
| 12 | Quadp_Quad_Commune_C4a      |  29 | ✅ | T1910 sancti chain: main-stem-only suppression + dirge skip + layer-aware leap suppression + back-walk |

**Closed: 11 / 12 clusters, 306 / 463 fail-days (66%).**

## Cluster 11 deferred — Phase 9 work

Pent19_23_SelfRef_R55 needs the commemoration-appendage logic from
Perl `propers.pl::oratio` lines 285-330 + `getcommemoratio()` + the
mass-context-commune-fallback-to-horas (`SetupString.pl:547-551`).
A first-pass implementation correctly resolves Hilarion's
"Intercéssio nos…" body via horas/Commune/C5b with N-substitution,
but interaction with `apply_world_mission_oratio` is non-trivial:
* For 1979-10-21 R55 (Pent20-0 + Hilarion 1.1): Perl shows ONLY
  Hilarion commemoration. Propaganda is suppressed.
* For 1985-10-20 R55 (Pent21-0 + Cantius 3.0): Perl shows ONLY
  Propaganda commemoration. Cantius is suppressed.
* For 1979-10-28 R55 (Pent21-0 + Vigilia Omnium SS 1.5): different
  pattern again — Perl shows only Vigil.

The R55 suppression rule is rank-based: Class III feasts (Duplex,
rank 3) on a I/II classis Sunday don't get a Mass commemoration
under R55 (only at Lauds), but Simplex feasts (rank 1.1) keep
their commemoration. Implementing this requires coordinated
edits across `apply_world_mission_oratio` and a new
`apply_sancti_commemoration_oratio`, plus access to the saint's
[Name] body for N-substitution. Total scope ≈ 200 lines plus tests.
A partial implementation (just the commemoration-body extraction)
landed locally but was reverted to avoid double-emit regressions.

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
