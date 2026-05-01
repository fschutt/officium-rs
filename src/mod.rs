//! Liturgical-calendar primitives, vendored from the working
//! `81f0b24` commit of the `divinum-officium-rs` AI port of the
//! Divinum Officium Perl source. The full crate has 59 compile
//! errors at HEAD; we only need date math (Easter, Advent, week
//! labels) for the dubia.cc /wip/calendar page, so we lift the
//! one file that produces clean output.
//!
//! Source: <https://github.com/fschutt/divinum-officium-rs/blob/81f0b24/src/date.rs>
//!
//! This module is *not* a fork-and-improve target. As `divinum-officium-rs`
//! converges on a working state, replace the path-vendored copy with a
//! proper `path = "../divinum-officium-rs"` dependency.

pub mod date;
pub mod sancti;
pub mod kalendaria;
// TODO Phase 11: delete precedence_legacy when /wip/calendar and /wip/missal
// switch to the canonical precedence::compute_office pipeline.
pub mod precedence_legacy;
pub mod missa;
pub mod translation;

// Phase 1 — pure-core types + corpus boundary + reform-layer model.
// These are the new boundary; Phases 3-5 plant the actual occurrence /
// precedence / mass_propers logic on top. Existing modules above stay
// in place (and feed the simplified `precedence::decide()` callsite in
// calendar.rs / missal.rs) until Phase 11 cuts the wire-in.
pub mod core;
pub mod corpus;
pub mod reform;

// Phase 3 — occurrence(): given a (date, rubric), pick the winner
// between the temporal and sanctoral cycles, plus any commemoration.
// Tridentine 1570 only at present; other rubrics panic with a marker.
pub mod occurrence;

// Phase 4 — precedence(): high-level orchestrator that calls
// `occurrence::compute_occurrence`, applies post-processing rules
// (rank parsing, season detection, color resolution, scriptura
// chaining), and produces the canonical `core::OfficeOutput`. Public
// entry point: `precedence::compute_office(input, corpus)`.
pub mod precedence;

// Phase 5 — Mass-propers resolver: takes an `OfficeOutput` and
// produces `MassPropers` (Introitus, Oratio, Lectio, Graduale,
// Evangelium, Offertorium, Secreta, Communio, Postcommunio, …).
// Pure string assembly over the corpus, no HTML, no globals.
// Public entry point: `mass::mass_propers(office, corpus)`.
pub mod mass;

// Phase 6 — Rust↔Perl regression machinery. Pure-functional HTML
// extractor + diacritic/punctuation normaliser + per-section
// comparator. Used by the `year-sweep` binary to produce green/
// yellow/red boards under `target/regression/`.
pub mod regression;

// Phase 6.5 — Mass-Ordinary prayer corpus (Prayers.txt). Holds the
// `[Gloria]`, `[Per Dominum]`, `[Dominus vobiscum]`, … bodies that the
// upstream renderer interpolates via `&Macro`/`$Macro` tokens. The
// macro expander in `mass.rs` consumes this; the regression
// comparator uses it to bring Rust output into shape parity with the
// rendered Perl HTML.
pub mod prayers;

// Phase 7 — Tridentine 1570 kalendar override
// (`Tabulae/Kalendaria/1570.txt`). Date → Sancti-stem map that
// supplies the right Tridentine variant of feasts (e.g. 01-23 →
// `Sancti/01-23o` Emerentiana instead of post-1570 Raymond) and
// pre-1911 commemoration pairings.
pub mod kalendarium_1570;
