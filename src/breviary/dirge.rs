//! Office of the Dead "dirge" predicate.
//!
//! Mirror of `vendor/divinum-officium/DivinumOfficium/Directorium.pm::dirge`.
//! Used at Vespera and Laudes to decide whether to substitute the
//! standard conclusion with the Vesperae Defunctorum / Officium
//! Defunctorum block.
//!
//! Drives:
//!   * `Vesperae Defunctorum` after Vespera on certain conventual
//!     days (per the Directorium).
//!   * `Officium Defunctorum` after Laudes on the same days.
//!   * Suppressed when today is itself an Office of the Dead.

use crate::breviary::horas::Hour;
use crate::core::Date;

/// What kind of dirge applies at this hour today?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirgeKind {
    None,
    /// Recite Vesperae Defunctorum after Vespera (today's office).
    VesperaeDefunctorum,
    /// Recite Officium Defunctorum after Laudes.
    OfficiumDefunctorum,
}

/// Compute the dirge kind for a given hour on a given date.
///
/// Mirrors `dirge($version, $hora, $day, $month, $year, $dioecesis)`.
/// Reads the Directorium calendar — a per-diocese list of
/// "anniversarius defunctorum" days that get the dirge appended.
pub fn dirge(
    _hour: Hour,
    _date: Date,
    _rubric: crate::core::Rubric,
    _diocese: &str,
) -> DirgeKind {
    // TODO(B17): port DivinumOfficium::Directorium::dirge.
    // The Directorium tables themselves need their own data pipeline
    // — sourced from `vendor/.../DivinumOfficium/Directorium/` (per-diocese
    // text files keyed by date). For B17 the simplest path is to
    // ship a baked `data/dirge_table.json` listing every (rubric,
    // diocese, date) → DirgeKind triple and look up here.
    unimplemented!("phase B17: dirge")
}
