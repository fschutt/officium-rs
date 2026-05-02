# officium-rs

Divinum Officium rubric core in pure Rust. Computes the Roman-rite
liturgical calendar and resolves Mass propers for any date under any
of five rubric layers — Tridentine 1570 → Rubrics 1960 (John XXIII) —
with 100% output parity against the upstream Perl implementation
across a year-sweep regression (21,900 cells × 5 rubrics).

**Demo:** <https://fschutt.github.io/officium-rs/> (coming soon)

## Status

- ✅ Calendar, occurrence, precedence, mass-propers resolution (Latin)
- ✅ All five rubric layers at 100% Perl parity
- ⏳ Full WASM build with embedded compressed corpus
- ⏳ no_std migration
- ⏳ Monastic rubric
- ⏳ Office hours (Vespers, Lauds, …) — only Mass today
- ⏳ Translations (English, German, …) — Latin only today

## Architecture

The crate exposes a `Corpus` trait + pure functions over it. The
default `BundledCorpus` reads from the JSON corpus shipped under
`data/` (embedded via `include_str!` for now; a follow-up commit
moves the runtime to compressed postcard bytes).

Rubric layers covered:

| layer            | year       | enum                                   |
| ---------------- | ---------- | -------------------------------------- |
| Tridentine 1570  | 1570–1909  | `Rubric::Tridentine1570`               |
| Tridentine 1910  | 1910–1955  | `Rubric::Tridentine1910`               |
| Divino Afflatu   | 1911–1955  | `Rubric::DivinoAfflatu`                |
| Reduced 1955     | 1955–1959  | `Rubric::Reduced1955`                  |
| Rubrics 1960     | 1960–      | `Rubric::Rubrics1960`                  |

## Usage

```rust
use officium_rs::{precedence::compute_office, mass::mass_propers,
                  corpus::BundledCorpus, core::*};

let corpus = BundledCorpus;
let input = OfficeInput {
    year: 2026, month: 5, day: 2,
    rubric: Rubric::Rubrics1960,
};
let office = compute_office(&input, &corpus);
let mass = mass_propers(&office, &corpus);
println!("{}", mass.introitus.body);
```

## Features

| feature       | what it gives you                                     |
| ------------- | ----------------------------------------------------- |
| `regression`  | (default) Rust↔Perl comparator + `year-sweep` binary  |

The `regression` feature is **native-only** — it shells out to the
upstream Perl runtime for reference-output diffs. Building for
`wasm32-unknown-unknown` with `regression` enabled triggers a
`compile_error!`.

## Regression harness

Run the year-sweep against the upstream Perl source:

```sh
git submodule update --init --recursive    # pulls vendor/divinum-officium-perl
cargo run --bin year-sweep --release -- --year 2026
```

Boards land under `target/regression/{slug}-{year}/board.html`.
Every cell should be green for all five rubrics.

## Provenance

This crate ports the Divinum Officium Perl implementation
(<https://github.com/DivinumOfficium/divinum-officium>) — vendored
as a git submodule under `vendor/divinum-officium-perl/` for
regression testing. All liturgical content remains under the
upstream's terms; this crate is the rubric *logic* in Rust.

## License

MIT. See [`LICENSE`](LICENSE).
