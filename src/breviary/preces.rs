//! Preces — Preces Feriales and Preces Dominicales.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials/preces.pl`:
//!
//! - `preces($item)` (line 7) — predicate; returns true if the active
//!   day calls for Preces Feriales or Dominicales (depending on the
//!   `$item` heading).
//! - `getpreces($hora, $lang, $is_dominicales)` (line 73) — emit the
//!   preces body.
//!
//! Eligibility is rubric-conditional and intricate. Roughly:
//!
//! - **Feriales** — said on most ferial days at Lauds & Vespera + at
//!   Prime, except: Easter Octave, days of I-classis rank, days
//!   carrying `Omit Preces` in their `[Rule]`, certain Octave days.
//! - **Dominicales** — said only at Prime / Compline on Sundays
//!   outside the festal seasons, plus on certain Saturdays.
//! - Both abolished under Rubrics 1960 except for Lent + Ember days.

use crate::breviary::horas::Hour;
use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Which preces (if any) apply at this hour today?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrecesKind {
    /// No preces — the section is omitted entirely.
    None,
    /// Preces Feriales.
    Feriales,
    /// Preces Dominicales.
    Dominicales,
}

/// Compute the preces kind for the active office at `hour`.
///
/// `section_label` is the upstream `#Preces ...` heading — typically
/// `"Preces Feriales"` or `"Preces Dominicales"`. The Perl entry uses
/// it to disambiguate the two flavours.
///
/// Mirror of `preces($item)` lines 7-72.
pub fn should_say_preces(
    _office: &OfficeOutput,
    _hour: Hour,
    _section_label: &str,
) -> PrecesKind {
    // TODO(B12): port specials/preces.pl:7-72.
    // Decision tree:
    //   1. Reject under R60 outside Lent + Ember days.
    //   2. Reject when [Rule] has `Omit Preces`.
    //   3. Reject when winner.rank.kind == DuplexIClassis or higher.
    //   4. Reject within Octaves except specific carve-outs.
    //   5. Otherwise return Feriales or Dominicales based on
    //      day_kind + section_label.
    unimplemented!("phase B12: should_say_preces")
}

/// Emit the preces body for the active hour.
///
/// Mirror of `getpreces($hora, $lang, $is_dominicales)` lines 73-105.
pub fn get_preces_body(
    _office: &OfficeOutput,
    _hour: Hour,
    _kind: PrecesKind,
) -> Vec<RenderedLine> {
    // TODO(B12): port specials/preces.pl:73-105. Reads
    // `Psalterium/Special/Preces.txt` for the per-hour body keyed by
    // `[Preces Feriales <Hora>]` / `[Preces Dominicales <Hora>]`.
    unimplemented!("phase B12: get_preces_body")
}
