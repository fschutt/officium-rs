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
