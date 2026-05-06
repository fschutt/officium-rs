//! Matins psalmody — psalm selection + antiphon set per nocturn.
//!
//! Mirror of `specmatins.pl`:
//! - `psalmi_matutinum($lang)` (line 228-458) — pulls per-day-of-week
//!   psalms from `Psalterium/Psalmi/Psalmi matutinum.txt`. Tridentine
//!   uses `[Day0]`–`[Day6]`; post-Divino uses the same blocks but
//!   selects a smaller set per ferial.
//! - `getantmatutinum($lang, $n)` (line 1813) — pulls the nocturn-N
//!   antiphon from the day file (with commune / tempora fallback).
//! - `ant_matutinum_paschal($lang)` (line 1555) — Easter-Octave
//!   ferial Matins antiphons (only one antiphon per nocturn under
//!   the Octave).
//!
//! Festal vs ferial dispatch:
//!   - **Festal** — fixed antiphons from the feast file (`Ant Matutinum
//!     1` … `Ant Matutinum 9`).
//!   - **Ferial** — per-day-of-week antiphons from the psalter index.
//!     1-class feasts always go festal; 2-class feasts go festal
//!     except on Octave-day ferials.

use crate::core::OfficeOutput;
use crate::breviary::psalter::PsalmRendered;

/// Pick the psalmody for one nocturn.
///
/// Mirror of `psalmi_matutinum($lang)` lines 228-458.
pub fn psalmi_matutinum(_office: &OfficeOutput, _nocturn_idx: u8) -> Vec<PsalmRendered> {
    // TODO(B19): port specmatins.pl:228-458. The richest single chunk
    // in this hour after lectiones; ~230 LOC of Perl with deep
    // ferial/festal/Pius-X branching.
    unimplemented!("phase B19: psalmi_matutinum")
}

/// Pull the nocturn-N antiphon set. Mirror of `getantmatutinum` line 1813.
pub fn get_ant_matutinum(_office: &OfficeOutput, _nocturn_idx: u8) -> Vec<String> {
    // TODO(B19): port specmatins.pl:1813+. Today the basic version
    // is `crate::horas::collect_nocturn_antiphons`.
    unimplemented!("phase B19: get_ant_matutinum")
}

/// Easter-Octave ferial Matins antiphons. Single antiphon for all
/// 9 psalms (because the ferial ranks at 6.0 and inherits the
/// festal-Sunday antiphon, but the psalmody is still ferial).
///
/// Mirror of `ant_matutinum_paschal` line 1555.
pub fn ant_matutinum_paschal(_office: &OfficeOutput) -> Option<String> {
    // TODO(B19): port specmatins.pl:1555-1603.
    unimplemented!("phase B19: ant_matutinum_paschal")
}
