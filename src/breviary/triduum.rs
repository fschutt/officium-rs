//! Triduum / Septuagesima predicates.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/horas.pl`:
//!
//! - `triduum_gloria_omitted()` (lines 226-235) — predicate; returns
//!   true when Gloria Patri at end of psalmody should be suppressed
//!   (Triduum Thu Compline → Sat Vespers).
//! - `Septuagesima_vesp()` (lines 216-221) — predicate; returns true
//!   when first Vespers of Septuagesima Sunday is being sung (drives
//!   the `&Benedicamus_Domino alleluja` swap).
//!
//! These are pure date / state predicates — no body emission. Both
//! consumed by `crate::breviary::postprocess::adjust_refs` and the
//! main walker.

use crate::breviary::horas::Hour;
use crate::core::OfficeOutput;

/// Should Gloria Patri at end of each psalm be omitted?
///
/// Returns true when:
///   - dayname[0] starts with `Quad6` (Holy Week), AND
///   - dayofweek > 3 (Thursday onwards), AND
///   - vespera != 1 (i.e. NOT first Vespers of the next day, which
///     would belong to a non-Triduum office).
///
/// Mirror of `triduum_gloria_omitted` lines 226-235.
pub fn triduum_gloria_omitted(_office: &OfficeOutput, _hour: Hour) -> bool {
    // TODO(B20): port horas.pl:226-235. Trivial date / state check.
    unimplemented!("phase B20: triduum_gloria_omitted")
}

/// Are we singing first Vespers of Septuagesima Sunday?
///
/// Returns true when:
///   - dayofweek == 6 (Saturday), AND
///   - hour == Vespera, AND either
///     - vespera == 1 AND dayname[0] starts `Quadp1` (Septuagesima Sun
///       prep on Saturday → first Vespers of Septuagesima), OR
///     - vespera == 3 AND cwinner ends with `Quadp1-0`.
///
/// Mirror of `Septuagesima_vesp` lines 216-221.
pub fn septuagesima_vesp(_office: &OfficeOutput, _hour: Hour) -> bool {
    // TODO(B20): port horas.pl:216-221. Trivial date / state check.
    unimplemented!("phase B20: septuagesima_vesp")
}
