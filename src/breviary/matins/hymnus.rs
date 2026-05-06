//! Matins-specific hymn selection.
//!
//! Mirror of `specmatins.pl::hymnusmatutinum($lang)` (lines 152-204).
//!
//! Distinguished from the general [`crate::breviary::hymnus`] because
//! Matins applies a "hymn-shift" / "hymn-merge" rule: certain feasts
//! (Christmas Day, Epiphany) push the previous day's Vespers hymn
//! into Matins of the following day, and the Vespers hymn rotates.
//! See `specmatins.pl::hymnshift` / `hymnshiftmerge` / `hymnmerge`
//! (referenced from `Directorium.pl`).

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Render the Matins hymn slot.
///
/// Mirror of `hymnusmatutinum($lang)` lines 152-204.
pub fn hymnus_matutinum(_office: &OfficeOutput) -> Vec<RenderedLine> {
    // TODO(B19): port specmatins.pl:152-204.
    // Composition:
    //   1. Apply hymn-shift rules (Christmas/Epiphany rotation).
    //   2. Look up `Hymnus Matutinum` per the proprium chain.
    //   3. Apply doxology selector.
    unimplemented!("phase B19: hymnus_matutinum")
}

/// Apply the hymn-shift rule for Christmastide / Epiphanytide.
/// Mirror of `hymnshift` / `hymnshiftmerge` / `hymnmerge` (referenced
/// in upstream Directorium.pl, called from this hour's hymn selector).
pub fn apply_hymn_shift(_office: &OfficeOutput, _hymn_body: &str) -> String {
    // TODO(B19): port the hymn-shift / hymn-merge tables.
    unimplemented!("phase B19: apply_hymn_shift")
}
