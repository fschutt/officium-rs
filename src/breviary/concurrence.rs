//! First-Vespers concurrence — produces `OfficeOutput.vespers_split`.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/horascommon.pl`:
//!
//! - `concurrence` (lines 810-1426) — the rubric-conditional ladder
//!   that decides whether tonight's Vespers belongs to today, to
//!   tomorrow, or splits "a capitulo de sequenti" between them.
//! - `extract_common` (lines 1433-1480) — helper used to lift the
//!   commune designation out of a winner's `[Rule]` body.
//! - `setsecondcol` (lines 1512-1525) — populates the second-column
//!   commemoration globals; in the Rust port commemorations are a
//!   field on `OfficeOutput`, not a global.
//!
//! ## How concurrence sits above occurrence
//!
//! `compute_occurrence` answers "what is the office of this day?".
//! Vespers and Compline straddle the calendrical day boundary, so
//! they may belong wholly to *tomorrow*, wholly to *today*, or be
//! split. `compute_concurrence(today, tomorrow, rubric)` answers that
//! one extra question and produces the [`crate::core::VespersSplit`].
//!
//! The occurrence engine itself doesn't change — concurrence is a
//! pure projection over two `OfficeOutput`s.
//!
//! ## Rubric ladder
//!
//! The Perl source is dense — three tables of "flattened ranks" per
//! era, with an extra branch for Sunday-vs-double, an extra branch
//! for vigils, and three different equal-rank tiebreakers (today
//! wins under 1960, tomorrow wins under Tridentine for I-class
//! feasts). Each rubric layer maps to a different ladder:
//!
//! | Rubric | Ladder |
//! |---|---|
//! | Tridentine 1570 | "Class minor flatten" — all minor doubles below 2.99 collapse to rank 2 for compare; equal ranks → tomorrow (the dignior) wins for I/II classis |
//! | Divino Afflatu 1911 | Sunday-vs-double has its own ladder; semiduplex Suffragium logic kicks in |
//! | Reduced 1955 | Octaves abolished, Suffragium dropped; otherwise as Divino |
//! | Rubrics 1960 | Equal ranks → today wins; "Vesperae infra octavam" suppressed |
//! | Monastic | Out of scope for first parity pass |
//!
//! See `BREVIARY_PORT_PLAN.md §4` for the slice plan.

use crate::core::{OfficeOutput, Rubric, VespersSplit};

/// Compute the first-Vespers split between today and tomorrow.
///
/// Returns `None` when today wholly owns Vespers (the common case —
/// no concurrence). Returns `Some(VespersSplit { … })` when tomorrow
/// outranks today (FirstVespers of tomorrow), today outranks tomorrow
/// (SecondVespers of today + commemoration of tomorrow), or the
/// flattened ranks tie (a-capitulo: psalmody from today, capitulum +
/// hymn + Magnificat from tomorrow).
///
/// Mirrors `concurrence()` lines 810-1426 in `horascommon.pl`. The
/// rubric layer is read from `today.rubric`.
pub fn compute_concurrence(
    _today: &OfficeOutput,
    _tomorrow: &OfficeOutput,
) -> Option<VespersSplit> {
    // TODO(B11): port horascommon.pl:810-1426 (~620 LOC). Reads:
    //   * today.rank.rank_num + today.commemoratio.rank_num
    //   * tomorrow.rank.rank_num + tomorrow.commemoratio.rank_num
    //   * today.rubric → drives the flatten-ladder
    //   * today.day_kind / season → drives Sunday-vs-double exception
    // Returns the structured `VespersSplit { split_at, from, to }`
    // (see `crate::core::VespersSplit`).
    unimplemented!("phase B11: full concurrence ladder")
}

/// Helper: rubric-conditional rank flattening. Takes a raw rank
/// number and applies the per-rubric "minor doubles flatten to 2"
/// rule. Mirrors the inline logic at `horascommon.pl:925-980` but
/// extracted so the per-rubric ladder is a pure function.
pub fn flatten_rank(rank_num: f32, rubric: Rubric) -> f32 {
    let _ = (rank_num, rubric);
    // TODO(B11): port the rank-flattening table from
    // horascommon.pl:925-980. Trivial given a rubric-keyed table.
    unimplemented!("phase B11: flatten_rank")
}

/// Decide whether the equal-rank case resolves in favour of today
/// ("dignior hodie") or tomorrow ("dignior cras"). The rubric ladder
/// is opposite under Tridentine vs 1960.
pub fn equal_rank_winner(_today: &OfficeOutput, _tomorrow: &OfficeOutput) -> EqualRankWinner {
    // TODO(B11): port the equal-rank tiebreaker from
    // horascommon.pl:990-1050.
    unimplemented!("phase B11: equal_rank_winner")
}

/// Result of the equal-rank tiebreaker. (`Today` is the modern-rubric
/// default; `Tomorrow` matches pre-1955 dignior-cras for I-class
/// feasts.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqualRankWinner {
    Today,
    Tomorrow,
}

/// Extract the commune designation from a winner's `[Rule]` body.
/// Mirror of `extract_common` (`horascommon.pl:1433-1480`).
///
/// Today the per-day commune chain is built by
/// `crate::horas::commune_chain` from the `[Rule]` body's `vide CXX`
/// directives. This fn extracts the *primary* commune (`C7a` / `C7` /
/// `C2-1`) for the rubric trace and the headline.
pub fn extract_common(_rule: &str) -> Option<String> {
    // TODO(B11): port horascommon.pl:1433-1480 (~50 LOC). Pulls
    // the first `vide CXX` / `ex CXX` directive out of the rule body
    // and returns the bare commune key.
    unimplemented!("phase B11: extract_common")
}

/// Stash the second-column commemoration data on `OfficeOutput`.
/// Mirror of `setsecondcol` (`horascommon.pl:1512-1525`). The Perl
/// version mutates `%winner2` / `%commune2` globals; the Rust port
/// folds the same data into `OfficeOutput.commemoratio` and the
/// per-section commune fallback chain.
pub fn populate_commemoration(_output: &mut OfficeOutput) {
    // TODO(B11): port horascommon.pl:1512-1525. Trivial — copies the
    // commemoration `FileKey`'s sections into a separate slot.
    unimplemented!("phase B11: setsecondcol")
}
