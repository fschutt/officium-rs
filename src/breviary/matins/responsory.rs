//! Matins responsory body emission (after each lectio).
//!
//! Mirror of `specmatins.pl::responsory_gloria($r, $lang)` (line 1482-1554).
//!
//! Each responsory has a body of the form:
//!
//! ```text
//! R. Body of responsory ¬ Last clause.
//! V. Versicle. * Last clause.
//! ¬ Glória Patri ... ¬ Last clause.   (final responsory of nocturn only)
//! ```
//!
//! The "Glória Patri" branch fires only on the final responsory of
//! each nocturn, not on every responsory. Under Easter Octave, all
//! responsories swap their Gloria Patri with `Alleluia, alleluia,
//! alleluia`.

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Apply the Gloria-Patri / Alleluia branch to a responsory.
///
/// Mirror of `responsory_gloria($r, $lang)` lines 1482-1554.
pub fn responsory_gloria(
    _office: &OfficeOutput,
    _responsory_body: &str,
    _is_final_in_nocturn: bool,
) -> Vec<RenderedLine> {
    // TODO(B19): port specmatins.pl:1482-1554.
    unimplemented!("phase B19: responsory_gloria")
}
