# officium-rs

[![Pages][pages-badge]][demo]
[![Regression][reg-badge]][reg-actions]

[pages-badge]: https://github.com/fschutt/officium-rs/actions/workflows/pages.yml/badge.svg
[reg-badge]:   https://github.com/fschutt/officium-rs/actions/workflows/regression.yml/badge.svg
[demo]:        https://fschutt.github.io/officium-rs/
[reg-actions]: https://github.com/fschutt/officium-rs/actions/workflows/regression.yml

Divinum Officium rubric core in pure Rust. Computes the Roman-rite
liturgical calendar and resolves Mass propers for any date under any
of five rubric layers — Tridentine 1570 → Rubrics 1960 (John XXIII) —
with 100% output parity against the upstream Perl implementation
across a year-sweep regression (21,900 cells × 5 rubrics).

**Demo:** <https://fschutt.github.io/officium-rs/> — picks any date,
any of five rubrics, and renders the full Tridentine Mass in the
browser. Latin Ordinary (Kyrie, Gloria, Credo, Sanctus, Pater Noster,
Agnus Dei, dialog versicles, inline rubrics) interleaved with the
day's Propers, all driven by a 907 KB WebAssembly bundle (~700 KB
gzip-served).

## Status

- ✅ Calendar, occurrence, precedence, mass-propers resolution (Latin)
- ✅ All five rubrics at **100%** parity for 2026 (the development
  reference year). Across the full ±50-year sweep (1976–2076 × 5
  rubrics × 365 days = 184 455 cells) the parity rate is **99.86%**;
  failures cluster on a handful of edge dates per rubric — see
  [`docs/REGRESSION_RESULTS.md`](docs/REGRESSION_RESULTS.md) and
  [`docs/UPSTREAM_WEIRDNESSES.md`](docs/UPSTREAM_WEIRDNESSES.md)
- ✅ WASM build (907 KB raw / ~700 KB gzip / ~660 KB brotli; bindgen
  API in [`src/wasm.rs`](src/wasm.rs))
- ✅ Full Mass renderer in the demo
- ✅ Multi-year regression CI — runs `year-sweep` against the
  upstream Perl across N years × 5 rubrics
  ([workflow][reg-actions])
- ⏳ `no_std` migration
- ⏳ Monastic rubric
- ⏳ Office hours (Vespers, Lauds, …) — only Mass today; see
  [`docs/BREVIARY_PORT_SCOPE.md`](docs/BREVIARY_PORT_SCOPE.md)
- ⏳ Translations (English, German, …) — Latin only today

[weird]: UPSTREAM_WEIRDNESSES.md

## Architecture

The crate exposes a `Corpus` trait + pure functions over it. The
default `BundledCorpus` reads from the JSON corpus shipped under
`data/` (embedded via `include_str!`); consumers can supply their own
impl for custom data sources.

Rubric layers covered:

| layer            | year       | enum                                   |
| ---------------- | ---------- | -------------------------------------- |
| Tridentine 1570  | 1570–1909  | `Rubric::Tridentine1570`               |
| Tridentine 1910  | 1910–1938  | `Rubric::Tridentine1910`               |
| Divino Afflatu   | 1939–1954  | `Rubric::DivinoAfflatu1911`            |
| Reduced 1955     | 1955–1959  | `Rubric::Reduced1955`                  |
| Rubrics 1960     | 1960–      | `Rubric::Rubrics1960`                  |

## Usage — native

```rust
use officium_rs::{
    core::{Date, Locale, OfficeInput, Rubric},
    corpus::BundledCorpus,
    precedence::compute_office,
    mass::mass_propers,
};

let corpus = BundledCorpus;
let input = OfficeInput {
    date:   Date::new(2026, 5, 2),
    rubric: Rubric::Rubrics1960,
    locale: Locale::Latin,
};

let office = compute_office(&input, &corpus);
println!("winner = {}", office.winner.render());
// → "Sancti/05-02"

let mass = mass_propers(&office, &corpus);
if let Some(intr) = &mass.introitus {
    println!("{}", intr.latin);
}
```

## Usage — WASM (browser)

After `wasm-pack build` (or `cargo build --target wasm32-unknown-unknown
--features wasm --no-default-features` + `wasm-bindgen`):

```html
<script type="module">
  import init, { compute_office_json } from './pkg/officium_rs.js';

  await init();
  const json = compute_office_json(2026, 5, 2, 'rubrics-1960');
  const office = JSON.parse(json);
  console.log(office.winner);   // "Sancti/05-02"
  console.log(office.color);    // "White"
</script>
```

V1 ships the `compute_office_json` resolver only — date + rubric in,
JSON description of the office out (winner path, color, season, rank,
commemorations). Full Mass-propers body assembly over WASM is V2.

See [`demo/`](demo/) for the live deployment source — vanilla HTML +
ES-module JS, no framework.

## Features

| feature       | what it gives you                                       |
| ------------- | ------------------------------------------------------- |
| `regression`  | (default) Rust↔Perl comparator + `year-sweep` binary    |
| `wasm`        | `wasm-bindgen` surface — see `src/wasm.rs`              |

The `regression` feature is **native-only** — it shells out to the
upstream Perl runtime for reference-output diffs. Building for
`wasm32-unknown-unknown` with `regression` enabled triggers a
`compile_error!`. Use `--no-default-features --features wasm` for
WASM builds.

## Regression harness

Run the year-sweep against the upstream Perl source (requires `perl5`
and the bundled CPAN deps; see `scripts/setup-divinum-officium.sh`):

```sh
git submodule update --init --recursive    # pulls vendor/divinum-officium

# Single year, single rubric — local development:
cargo run --bin year-sweep --release -- \
    --year 2026 --rubric 'Rubrics 1960 - 1960'

# Multi-year range — for proving long-window parity:
cargo run --bin year-sweep --release -- \
    --years 2016:2036 --rubric 'Tridentine - 1570' --strict
```

Boards land under `target/regression/{slug}-{year}/board.html`.
Every cell green = parity; the published baseline is at the SHA in
`scripts/divinum-officium.pin` (April 2026 upstream).

Continuous integration runs the same sweep weekly across all five
rubrics against a configurable year range — see
[`.github/workflows/regression.yml`](.github/workflows/regression.yml).
The default `workflow_dispatch` window is ±10 years (2016 – 2036);
wider runs (e.g. `1976:2076` for the full ±50) can be triggered
manually. Perl-rendered HTML is cached by upstream-pin SHA so re-runs
against the same upstream commit are fast.

## Provenance

This crate ports the Divinum Officium Perl implementation
(<https://github.com/DivinumOfficium/divinum-officium>) — vendored as a
git submodule under `vendor/divinum-officium/` for regression testing.
All liturgical content remains under the upstream's terms; this crate
is the rubric *logic* in Rust.

## License

MIT for the Rust code itself. See [`LICENSE`](LICENSE). The vendored
upstream Perl tree under `vendor/divinum-officium/` carries its own
license terms; data files derived from that tree are subject to them.
