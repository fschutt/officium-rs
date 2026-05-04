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
13. **Spin-out / V1** — Extracted from the website monorepo via
    `git filter-repo`; framed as a standalone crate with a WASM bindgen
    surface and a GitHub Pages demo. 4-of-5 rubrics at 100% parity,
    R60 at 99.7% (one known occurrence-resolution gap, see
    [`docs/UPSTREAM_WEIRDNESSES.md`](docs/UPSTREAM_WEIRDNESSES.md) §35).
14. **V2 — postcard transcoding.** `build.rs` reads source JSON at
    compile time and emits postcard binaries to `OUT_DIR`; lib loads
    via `include_bytes!` + `postcard::from_bytes`. Drops the
    `serde_json` runtime dep from the WASM bundle (gated behind the
    `regression` feature). Shared struct definitions live in
    `src/data_types.rs`, `#[path]`-included by `build.rs`. WASM
    bundle: 3.4 → 2.8 MB raw (16% smaller). Served gzip is slightly
    larger (926 → 1020 KB) since postcard's tighter packing leaves
    less redundancy for HTTP-layer compression — the net win is
    faster runtime decode + smaller compiled-code footprint.
15. **V3 — brotli precompression + full Mass renderer + multi-year
    regression CI.**
    - Compression bench (`docs/COMPRESSION_BENCH.md`) ran 8 encoders
      across each input. `postcard + brotli` won on total bundle
      (data + decoder code): WASM dropped 2.88 MB → **907 KB raw**,
      ~1.0 MB → **~700 KB** gzip-served from Pages. Brotli is
      pure-Rust via `brotli-decompressor` (~30 KB compiled).
    - Full Mass renderer on the demo: new `compute_mass_json` WASM
      surface returns the propers + `gloria`/`credo`/`prefatio_name`
      rules; `demo/ordo.js` is the static Latin Tridentine Ordinary
      transcribed from upstream `Ordo.txt`; `demo/render.js`
      interleaves them in liturgical order with role tags (S./M./V./R.),
      red rubrics, citation headers, separator marks, conditional
      Gloria/Credo, and Ite-Missa-Est/Benedicamus-Domino branching.
    - Multi-year regression CI: `year-sweep --years FROM:TO --strict`,
      matrix on 5 rubrics, weekly schedule, Perl-output cache keyed
      by upstream pin SHA. First green run: 1976:2076 (101 years × 5
      rubrics × ~365 days = ~184 000 cells) all 100%, validating that
      the Rust core can replace the Perl reference for any date in
      the modern era.

## V4 backlog

- **`no_std` + `alloc` migration.** Most of the crate is already
  allocation-only; the regression module is the one place that uses
  `std` heavily, and it's already gated `cfg(not(target_arch =
  "wasm32"))`. Remaining
  work is replacing `String` with `alloc::string::String` and
  collections imports.
- **Mass-propers body assembly over WASM.** V1 returns office winner
  + commemoration codes only; full Latin body assembly via
  `mass_propers` is the next WASM API addition.
- **Office hours.** Currently Mass-only; Vespers / Lauds / Matins
  resolution shares ~80% of the rubric layer but pulls from
  `vendor/divinum-officium/web/cgi-bin/horas/officium.pl` not
  `missa/missa.pl`.
- **Monastic + non-Latin translations.** Both deferred from V1.
