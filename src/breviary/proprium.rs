//! Proper-section resolver — Office side.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials.pl`:
//!
//! - `getproprium($name, $lang, $type, $build)` (lines 443-521) —
//!   four-level fallback: winner → commune → tempora → psalterium-special.
//! - `getanthoras($lang)` (lines 543-573) — antiphons for the small hours.
//! - `getantvers($section, $key, $lang)` (lines 575-622) — antiphon
//!   + versicle pair lookup.
//! - `getseant($section, $key, $lang)` (lines 624-638) — seasonal
//!   antiphon override.
//! - `getfrompsalterium($section, $lang)` (lines 640-657) — last-resort
//!   `Psalterium/Special/<HourClass> Special.txt` fallback.
//!
//! Today the working 1-hop resolver is `crate::horas::find_section_in_chain`
//! plus the `commune_chain` walker. This module is the home for the
//! full 4-level chain post-B10.

use crate::breviary::horas::Hour;
use crate::core::OfficeOutput;
use crate::data_types::HorasFile;

/// One result of a proper-section resolution.
#[derive(Debug, Clone)]
pub struct ProperResolution {
    /// The resolved body.
    pub body: String,
    /// Which fallback level produced the body (winner / commune /
    /// tempora / psalterium-special). Drives the `setbuild` trace.
    pub source: ProperSource,
    /// The `FileKey`-style path of the source file.
    pub source_key: String,
}

/// Which fallback level produced a proper resolution. Mirrors the
/// `$c` (comment-index) return value from `getproprium` — Perl's
/// numeric code for the source-of-text annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProperSource {
    Winner,           // 0 — proper to the day's winner
    Commune,          // 1 — pulled from `Commune/Cxx[a-z]`
    Tempora,          // 2 — seasonal fallback (`Tempora/...`)
    PsalteriumSpecial, // 3 — `Psalterium/Special/*.txt`
    Empty,            // section absent from every fallback
}

/// Walk the four-level fallback chain for `section` against the
/// office's resolution chain.
///
/// Mirrors `getproprium($section, $lang, $type, $build)` lines 443-521.
/// Returns the first non-empty body found, plus its source level.
pub fn get_proprium(
    _office: &OfficeOutput,
    _section: &str,
) -> ProperResolution {
    // TODO(B10): port specials.pl:443-521 (~80 LOC of Perl). Walks:
    //   1. winner sections — `$w{$section}` (today's
    //      `crate::horas::find_section_in_chain` first hit)
    //   2. commune sections — `$c{$section}` (the `vide CXX` fallback)
    //   3. tempora sections — pulled via `gettempora($section)`
    //   4. `Psalterium/Special/<HourClass> Special.txt` (last resort)
    //
    // The `@`-redirect form (`@Path:Section`) is handled at level 1
    // via `crate::setupstring::resolve_section`.
    unimplemented!("phase B10: getproprium 4-level fallback")
}

/// `getanthoras($lang)` (specials.pl:543-573) — produce the antiphon
/// set for the small hours (Tertia / Sexta / Nona). Pulls from the
/// `Ant 1` / `Ant 2` / `Ant 3` keys of the day file, with the per-
/// nocturn / per-hour fallbacks the Perl walker honours.
pub fn get_ant_hours(
    _office: &OfficeOutput,
    _hour: Hour,
) -> Vec<String> {
    // TODO(B10): port specials.pl:543-573. Returns 3 antiphons
    // (one per psalm at the small hour) or 5 (Lauds/Vespers).
    unimplemented!("phase B10: getanthoras")
}

/// `getantvers($section, $key, $lang)` (specials.pl:575-622) —
/// resolve an antiphon + versicle/response pair (used by Capitulum
/// + Magnificat-antiphon slot).
pub fn get_ant_vers(
    _office: &OfficeOutput,
    _section: &str,
    _key: &str,
) -> Option<(String, String)> {
    // TODO(B10): port specials.pl:575-622.
    unimplemented!("phase B10: getantvers")
}

/// `getseant($section, $key, $lang)` (specials.pl:624-638) —
/// seasonal antiphon override. When the active season has its own
/// antiphon set keyed in `Psalterium/Special/<HourClass> Special.txt`
/// it wins over the per-day antiphon.
pub fn get_seasonal_antiphon(
    _office: &OfficeOutput,
    _section: &str,
    _key: &str,
) -> Option<String> {
    // TODO(B10): port specials.pl:624-638.
    unimplemented!("phase B10: getseant")
}

/// `getfrompsalterium($section, $lang)` (specials.pl:640-657) —
/// last-resort lookup in the `Psalterium/Special/<HourClass>` files
/// (`Major Special.txt`, `Minor Special.txt`, `Matutinum Special.txt`,
/// `Prima Special.txt`).
pub fn get_from_psalterium(
    _hour: Hour,
    _section: &str,
) -> Option<String> {
    // TODO(B10): port specials.pl:640-657. Wraps `crate::horas::lookup`
    // against the right `Psalterium/Special/...` key.
    unimplemented!("phase B10: getfrompsalterium")
}

/// Saint-name placeholder substitution. `replaceNdot` (specials.pl:782).
///
/// The working implementation lives privately in
/// `crate::horas::substitute_saint_name`. After M7 of the migration
/// (see `BREVIARY_PORT_PLAN.md §6`) the body moves here and the
/// `crate::horas` private fn is removed.
pub fn replace_n_dot(_body: &str, _name: Option<&str>) -> String {
    // TODO(M7): move `crate::horas::substitute_saint_name` here. Until
    // then, callers that need the working impl reach into
    // `crate::horas` directly via the `compute_office_hour` walker.
    unimplemented!("phase M7: replace_n_dot moved from crate::horas")
}

#[allow(dead_code)]
fn _placeholder_uses_horas_file(_f: &HorasFile) {
    // Suppress unused-import lint until the real signatures land.
}
