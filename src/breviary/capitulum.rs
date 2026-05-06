//! Chapter (Capitulum), short responsory, versicle / response.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials/capitulis.pl`:
//!
//! - `capitulum_major($lang)` (line 5) — Lauds / Vespera capitulum +
//!   short responsory + versicle.
//! - `monastic_major_responsory($lang)` (line 37) — Monastic-only
//!   long responsory at Lauds / Vespera. Out of scope for first
//!   parity pass; stub here so the dispatcher in
//!   `crate::breviary::specials` compiles.
//! - `postprocess_short_resp_gabc(@$lang)` (line 124) — GABC chant
//!   tone post-processing for short responsories. Out of scope.
//! - `minor_reponsory($lang)` (line 159, sic — Perl's spelling) — short
//!   responsory body for the minor hours.
//! - `capitulum_minor($lang)` (line 227) — Tertia / Sexta / Nona /
//!   Compline capitulum + (short responsory at T/S/N) + versicle.

use crate::breviary::horas::Hour;
use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Lauds / Vespera capitulum + short responsory + versicle bundle.
///
/// Mirror of `capitulum_major` line 5.
pub fn capitulum_major(_office: &OfficeOutput, _hour: Hour) -> Vec<RenderedLine> {
    // TODO(B16): port specials/capitulis.pl:5-36.
    // Composition:
    //   1. Lookup `Capitulum $hora` via proprium chain
    //   2. Lookup `Responsory $hora` (the short responsory)
    //   3. Lookup `Versum $hora` (versicle/response)
    //   4. Apply Paschal-Alleluia injection (postprocess_short_resp)
    //   5. Apply rubric-conditional Gloria Patri at responsory close
    unimplemented!("phase B16: capitulum_major")
}

/// Monastic long-responsory at Lauds / Vespera. Out of scope for
/// first parity pass.
///
/// Mirror of `monastic_major_responsory` line 37.
pub fn monastic_major_responsory(
    _office: &OfficeOutput,
    _hour: Hour,
) -> Option<Vec<RenderedLine>> {
    // TODO(M-future): port specials/capitulis.pl:37-123. Roman parity
    // pass returns None unconditionally.
    None
}

/// Short responsory body for Tertia / Sexta / Nona.
///
/// Mirror of `minor_reponsory` line 159 (note the Perl spelling).
pub fn minor_responsory(_office: &OfficeOutput, _hour: Hour) -> Vec<RenderedLine> {
    // TODO(B16): port specials/capitulis.pl:159-226.
    unimplemented!("phase B16: minor_responsory")
}

/// Capitulum + minor responsory + versicle for Tertia / Sexta / Nona /
/// Compline.
///
/// Mirror of `capitulum_minor` line 227.
pub fn capitulum_minor(_office: &OfficeOutput, _hour: Hour) -> Vec<RenderedLine> {
    // TODO(B16): port specials/capitulis.pl:227-254.
    // Composition mirrors capitulum_major minus the long-responsory
    // branch.
    unimplemented!("phase B16: capitulum_minor")
}
