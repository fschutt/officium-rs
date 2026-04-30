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
pub mod precedence;
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
