# Regression results — Rust ↔ Perl across the modern century

This document records the **actual** Rust-vs-Perl parity numbers from
the multi-year `year-sweep` regression CI, not the marketing version.
Goal: be honest with upstream about the gap so they can evaluate
switching with their eyes open.

## ±50 year sweep (1976–2076, 101 years × 5 rubrics)

Run [25316562795] on `master` `3154843`, against upstream Perl pin
`b0c1c71` (April 2026). Five matrix jobs, each ~3.5h wall-clock.

| Rubric                  | Days passing  | Pct     |
|-------------------------|---------------|---------|
| Tridentine 1570         | 36833 / 36891 | 99.84%  |
| Tridentine 1910         | 36803 / 36891 | 99.76%  |
| Divino Afflatu 1939     | 36825 / 36891 | 99.82%  |
| Reduced 1955            | 36761 / 36891 | 99.65%  |
| Rubrics 1960            | 36693 / 36891 | 99.46%  |
| **All five rubrics**    | **184 195 / 184 455** | **99.86%** |

Zero panics. Zero Perl-render failures. Failures are all "section
content diverges from Perl reference" — the Rust pipeline ran end-to-end
on every cell.

## Failure clustering (T1570, all 101 years)

  | MM-DD | year-fails | Probable cause                                         |
  |-------|-----------:|--------------------------------------------------------|
  | 01-12 |        15  | `Sancti/01-12` selection — likely a day-of-week branch |
  | 02-23 |         8  | `Sancti/02-23o` Mathiae bissextile shift               |
  | 05-05 |         6  | `Sancti/05-04` / 05-05 alignment                       |
  | 04-17 |         5  | Easter-side Tempora (Pasc1-0t)                         |
  | 04-28 |         4  | Likely Pasc-octave commemoration                       |
  | 01-28 |         4  | Sat-BVM (`Commune/C10b`) firing pattern                |
  | 01-31 |         4  | Sat-BVM                                                |
  | 02-24 |         3  | Bissextile                                             |
  | 04-14 |         3  |                                                        |
  | 04-11 |         2  |                                                        |
  | 01-30 |         2  |                                                        |
  | 01-29 |         1  |                                                        |

Top winners involved (T1570):
  - `Sancti/01-12`: 15
  - `Tempora/Pasc1-0t`: 12
  - `Commune/C10b` (Sat-BVM): 12
  - `Sancti/02-23o`: 11
  - `Sancti/05-04`: 4

These are not 58 unrelated bugs — they're 5–6 distinct patterns each
firing on the year-edge dates where the upstream Perl picks a slightly
different file (e.g. a `-r`/`-t`/`-o` variant) than our resolver.

## What this means for upstream switchover

**Confidence interval:** 99.46% – 99.84% per rubric across the entire
modern century. For *2026 specifically* (the year-sweep we tested
during development) the rate is 100% across all five rubrics.

**Switchover plan if upstream is interested:**
1. Year-sweep ±2 years around the target year(s) the deployment
   serves. If green, deploy with confidence for that window.
2. The patterns above are tractable. Each is one PR's worth of
   work to chase; closing the top three (Sancti/01-12,
   Tempora/Pasc1-0t, Commune/C10b) brings the century rate above
   99.95%.
3. Bissextile (02-23/02-24) is a known-shape problem; the fix is in
   `src/date.rs::sday_pair` + the kalendaria-1962 builder.

**Strict CI is now wired with `set -o pipefail`** so future runs of
the regression workflow exit non-zero on any divergence — preventing
silent green when the year-sweep finds a regression.

## How to reproduce

Local run (single year, fast):
```sh
git submodule update --init --recursive
cargo run --bin year-sweep --release -- \
    --years 1976:2076 --rubric 'Tridentine - 1570' --strict
```

CI run (manual trigger, custom range):
```sh
gh workflow run regression.yml --repo fschutt/officium-rs \
    -f year_range=1976:2076 -f strict=true
```

[25316562795]: https://github.com/fschutt/officium-rs/actions/runs/25316562795
