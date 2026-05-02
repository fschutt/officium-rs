# History

A commit-by-commit narrative of how this port reached 100% parity
against the upstream Perl Divinum Officium implementation.

The full story is told in the git log itself — `git log --oneline`
shows ~140 commits grouped roughly by phase below. This document
narrates the **interesting** ones: discoveries, debugging dead-ends,
reverts, upstream bugs found.

> 🚧 This document is a stub. The full narrative lands in a follow-up
> commit. For now, see:
> - [`docs/DIVINUM_OFFICIUM_PORT_PLAN.md`](docs/DIVINUM_OFFICIUM_PORT_PLAN.md) — the phase plan
> - [`docs/UPSTREAM_WEIRDNESSES.md`](docs/UPSTREAM_WEIRDNESSES.md) — the upstream-bug catalog
> - `git log --oneline` for the chronological view

## Phases (rough)

1. **Phase 0** — Vendor Perl + CLI render harness
2. **Phase 1** — Pure-core types + `Corpus` trait skeleton
3. **Phase 2** — Date math (Easter, Advent, week labels)
4. **Phase 3** — `occurrence()` for Tridentine 1570
5. **Phase 4** — `precedence()` orchestrator
6. **Phase 5** — `mass_propers()` resolver
7. **Phase 6** — Regression harness + `year-sweep` board
8. **Phase 6.5** — Macro corpus + comparator overhaul
9. **Phase 7** — Sancti / Tempora / Transfer table mechanics
10. **Phase 7+** — Layered orthography, `(rubrica X)` predicates, Triduum
11. **Phase 8-10** — Per-rubric reform layers (T1910 → DA → R55 → R60)
12. **Phase 11** — Wire-in to dubia.cc /wip/calendar + /wip/missal
