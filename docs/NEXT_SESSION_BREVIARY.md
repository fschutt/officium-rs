# Next session — drive the Breviary port to Perl-parity

**Goal:** drive the Office (breviary) renderer to 100% Perl-parity
across all 5 rubric layers × 1976-2076 — same exit criterion the
Mass side just hit (commit `7216ae2`, 184,455 / 184,455 days).

## Starting state (2026-05-07)

* **Mass side:** 100% across T1570/T1910/DA/R55/R60 × 1976-2076.
  Frozen unless a future fix regresses — gate with the existing
  cluster-verify scripts.
* **Breviary side:** B1-B7 done (corpus loader, hour walker,
  Vespera + Lauds + Prima + Tertia/Sexta/Nona + Compline + Matutinum,
  Te Deum), B8 done (regression plumbing + auto day-key + persistent
  driver), B9 in-flight (the 100-yr office sweep, currently at
  62.47% on Vespera 2026 Oratio — see baseline below).
* **Persistent Perl driver:** already hour-aware. The same
  `scripts/persistent_driver.pl` loads both `missa.pl` and
  `officium.pl` once at startup and forks per render. The
  `office_sweep` binary uses thread-local `MISSA_DRIVER` /
  `OFFICIUM_DRIVER` cells dispatched by `ScriptType::for_hour`. SHA-
  keyed gzip disk cache works for both. So you do NOT need to build
  a fast-iteration loop — it exists.

## Required reading before starting

In this order:

1. **`docs/MASS_PORT_LESSONS.md`** — 20 gotchas from the Mass
   cluster-closure loop. Most apply to the breviary because
   Mass + Office share helper modules (`Directorium`, `SetupString`,
   `horascommon`) and data files (Kalendaria, Transfer/Stransfer,
   Sancti/, Tempora/, Commune/). Especially:
     - #1 Perl `transfered()` substring-regex
     - #2 `[Section] (rubrica X)` annotation strip on chase
     - #3 Kalendar key vs file stem
     - #4 Layer-aware leap-Feb-23 filter-1 / filter-2
     - #5 Festum Domini precedence + RG 15
     - #15 missa-uses-horas commune fallback (now reversed:
       horas IS the canonical commune source)
2. **`docs/BREVIARY_PORT_PLAN.md`** — module map for B10-B20 and
   the Perl files each Rust module mirrors. Respect this layout.
3. **`docs/BREVIARY_REGRESSION_RESULTS.md`** — running tally of
   slice gains (slice 1: 26.67% → slice 8: 63.33% → slice 10:
   69.64% aggregate across 8 hours). Continue this.
4. **`docs/CLUSTER_PROGRESS.md`** — the loop structure that drove
   the Mass to 100%. Mirror the per-iteration template.

## Iteration loop (mirror of the Mass side)

Each iteration:

1. Run a focused slice:
     ```
     cargo run --release --bin office_sweep -- \
         --year YYYY --hour HOUR --rubric "RUBRIC NAME"
     ```
   Start with the lowest-pass-rate hour for the active rubric to
   maximise blast-radius per fix.
2. Dump a representative failing cell — note the date, hour,
   winner_perl, the `Differ` / `RustBlank` taxon, and the section
   that fails.
3. Locate the upstream Perl rule:
     - `vendor/divinum-officium/web/cgi-bin/horas/horascommon.pl`
       for occurrence + precedence.
     - `vendor/divinum-officium/web/cgi-bin/horas/officium.pl`
       for the hour-walking driver.
     - `vendor/divinum-officium/web/cgi-bin/horas/specials.pl`
       for hour-specific specials (Te Deum, Athanasian Creed,
       seasonal antiphon swaps).
     - `vendor/divinum-officium/web/cgi-bin/DivinumOfficium/`
       for `Directorium`, `SetupString`, etc. — same modules
       the Mass renderer uses.
4. Apply a narrow Rust-side fix in `src/horas.rs` /
   `src/breviary/*` / `src/setupstring.rs` / `src/kalendaria*.rs`.
   If too big: scope as a B-numbered slice and document in
   `BREVIARY_PORT_PLAN.md`.
5. Verify:
     - `cargo test --release` (must pass).
     - Re-run the same `office_sweep` slice.
     - Pick 2-3 already-fixed slices and re-run them as no-
       regression checks.
6. Commit + push.
7. Append a row to `BREVIARY_REGRESSION_RESULTS.md`.

## Concrete starting target

The B9 sweep at 62.47% Vespera-Oratio for 2026 has 121 differs +
15 rust-blanks. Suggested first slice:

```
cargo run --release --bin office_sweep -- \
    --year 2026 --hour all --rubric "Tridentine - 1570"
```

The aggregate report will show which hour is worst across all 8.
Lift that hour first (probably Matutinum, which has the densest
content). Dump 2-3 representative fails per failing hour, group
them into a "cluster" mirroring the Mass-side cluster table, and
start chipping.

The Mass loop ran with `/loop` cron `7,27,47 * * * *` — same cadence
works here. Per-iteration Perl driver is warm (forked from
pre-loaded parent), so cycle time is cheap.

## Final exit criterion

A fresh `office_sweep --years 1976:2076 --hour all --rubric R`
returns 100% on every (R, hour) pair in:

* T1570
* T1910
* DA (1939)
* R55
* R60

across all 8 canonical hours. That's 5 × 8 × 36,891 = ~1.5 M cell
comparisons. Persistent driver + warm cache + Rayon-per-year ⇒
runtime should be ~10-15 min per (R, hour) cold; sub-minute warm.

## Constraints inherited from CLAUDE.md memory

* Always `master` branch (never `main`) — the deploy is wired
  to `master` and the Pages URL hard-codes that name.
* Work the layered-reform model: 1570 baseline + composable
  reform layers (1910 → DA 1911 → R55 → R60). Pure functions, no
  globals. Perl vendor stays in `vendor/divinum-officium/` (gitignored
  apart from the submodule pointer).
* DiPippo's calendar > Perl when the two disagree. Perl's behaviour
  is the regression oracle for body content; DiPippo's calendar is
  the precedence oracle when ambiguous.
* No `--no-verify`, no commit hook bypass.
* No "pre-existing failure" notes in commit messages — fix or
  document the cluster, don't note-and-skip.
* Frame remaining residuals as "the work to do," not as a
  satisfied 99.X%.

## What I'd suggest the new session NOT do

* Don't refactor the existing Mass code. It's at 100% and the
  fixes are non-obvious; touching them risks regression.
* Don't try to build a new Perl driver — the existing one is
  dual-mode and works for both.
* Don't widen any cluster fix without no-regression checking 3
  prior closed clusters first. The Mass loop hit several
  "I'll just generalise" moments that broke 30+ days at once
  before being narrowed back.

## Useful commands cheat sheet

```bash
# baseline a year (any rubric)
cargo run --release --bin office_sweep -- --year 2026 --hour Vespera

# all 8 hours in one run
cargo run --release --bin office_sweep -- --year 2026 --hour all

# 100-year sweep when ready
cargo run --release --bin office_sweep -- --years 1976:2076 --hour all \
    --rubric "Tridentine - 1570" --quiet

# inspect a single cell
cargo run --release --bin office_sweep -- --date 12-25-2026 \
    --hour Matutinum --rubric "Tridentine - 1570"

# full Mass-side regression sanity (do NOT skip — every breviary
# fix that touches setupstring/kalendaria can regress mass)
cargo run --release --bin year-sweep -- --years 1976:2076 \
    --rubric "Rubrics 1960 - 1960" --quiet
```

Latest commits when this prompt was written: `da35a45` (lessons
doc) → builds on `7216ae2` (the 100% milestone).
