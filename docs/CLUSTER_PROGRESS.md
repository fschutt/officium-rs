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
|  7 | T1910_Annunciation          |   5 | ⏳ | needs T1910 Sunday-vs-Sancti precedence |
|  8 | T1910_Cathedra_Matthias     |   9 | ⏳ | needs T1910 Cathedra Petri Romae rule |
|  9 | DA_SeptEmbersCross          |  14 | ⏳ | needs DA Sept-Embers/Cross precedence |
| 10 | R55_SeptEmbersCross         |  14 | ⏳ | needs R55 Sept-Embers/Cross precedence |
| 11 | Pent19_23_SelfRef_R55       |  15 | ⏳ | needs R55 rank-aware commemoration suppression |
| 12 | Quadp_Quad_Commune_C4a      |  29 | ⏳ | needs T1910 commune-redirect for Quadp ferias |

**Closed: 6 / 12 clusters, 207 / 463 fail-days (45%).**

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
