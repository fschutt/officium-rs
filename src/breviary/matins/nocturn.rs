//! Single-nocturn renderer.
//!
//! Mirror of `specmatins.pl::nocturn` (lines 205-227).
//!
//! One nocturn = 3 (or 4 under some schemata) antiphons + 3 (or 6 in
//! Sunday-feria-of-9-lectiones) psalms + versicle + 3 lectiones.

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Render nocturn `n` (1, 2, or 3) of Matins.
///
/// Mirror of `nocturn` lines 205-227.
pub fn render_nocturn(_office: &OfficeOutput, _nocturn_idx: u8) -> Vec<RenderedLine> {
    // TODO(B19): port specmatins.pl:205-227. Composes:
    //   1. Antiphon set for this nocturn (via psalmody::ant_matutinum).
    //   2. Psalm set (via psalmody::psalmi_matutinum).
    //   3. Versicle/response close to the psalmody.
    //   4. 3 Lectiones with intervening Responsories
    //      (lectiones::render_lectio_trio).
    unimplemented!("phase B19: render_nocturn")
}
