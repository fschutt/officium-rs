//! Scripture-rotation rule: when a feast displaces a Sunday Matins,
//! what gets shifted where?
//!
//! Mirror of `specmatins.pl`:
//! - `initiarule()` (line 1604) — top-level decision: does the
//!   displaced Sunday's first-nocturn scripture shift to the next
//!   ferial?
//! - `resolveitable($i, $r)` (line 1622) — table of "which Sunday
//!   would have been read on which day".
//! - `tferifile($d, $m, $y)` (line 1716) — find the Tempora ferial
//!   file that should pick up the displaced lectio.
//! - `StJamesRule($d, $m, $y)` (line 1738) — special carve-out for
//!   the St James apostolic letter rotation.
//! - `prevdayl1($d, $m, $y)` (line 1772) — previous-day-with-Lectio1
//!   look-back; some ferial Mondays carry the Sunday's scripture
//!   that didn't get read.

use crate::core::Date;

/// Decide whether scripture rotation fires today.
///
/// Mirror of `initiarule()` lines 1604-1621.
pub fn initiarule_for_date(_date: Date) -> bool {
    // TODO(B19): port specmatins.pl:1604-1621. Returns true on the
    // ferials of the weeks where this rotation matters (post-
    // Epiphany, post-Pentecost autumn weeks).
    unimplemented!("phase B19: initiarule_for_date")
}

/// Look up the displaced-Sunday → ferial-target table.
///
/// Mirror of `resolveitable($i, $r)` lines 1622-1715.
pub fn resolve_i_table(_i: u8, _r: u8) -> Option<String> {
    // TODO(B19): port specmatins.pl:1622-1715. ~95 LOC of Perl
    // table data — could be moved to a JSON build-time data file.
    unimplemented!("phase B19: resolve_i_table")
}

/// Find the Tempora ferial file that should absorb today's
/// displaced lectio. Mirror of `tferifile` line 1716.
pub fn tferi_file(_date: Date) -> Option<String> {
    // TODO(B19): port specmatins.pl:1716-1737.
    unimplemented!("phase B19: tferi_file")
}

/// St James apostolic letter rotation carve-out. Mirror of
/// `StJamesRule` line 1738.
pub fn st_james_rule(_date: Date) -> bool {
    // TODO(B19): port specmatins.pl:1738-1771.
    unimplemented!("phase B19: st_james_rule")
}

/// Previous-day-with-Lectio1 look-back. Mirror of `prevdayl1`
/// line 1772.
pub fn prev_day_l1(_date: Date) -> Option<Date> {
    // TODO(B19): port specmatins.pl:1772-1789.
    unimplemented!("phase B19: prev_day_l1")
}
