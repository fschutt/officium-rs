//! Martyrology block for Prime.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials/specprima.pl::martyrologium`
//! (lines 138-228) plus the data files under
//! `vendor/divinum-officium/web/www/horas/Latin/Martyrologium/`.
//!
//! The Martyrologium is its own corpus — one file per civil day
//! (`01-01.txt` … `12-31.txt`) plus a few movable-feast supplements
//! (`Pasc.txt` for the days of Easter Octave, `Pent.txt` for Pentecost
//! Octave, etc.). Each day's body has the format:
//!
//! ```text
//! Quarto Kalendas Junii. Luna {luna}.
//! ! Romae sancti …, Apostoli …
//! …
//! ```
//!
//! plus the trailing `&Pretiosa` macro that closes the block.

use crate::core::{Date, OfficeOutput};
use crate::ordo::RenderedLine;

/// Render the full martyrology block for a date. Composes:
///   1. Date heading (`Quarto Kalendas Junii. Luna {luna}.`).
///   2. Body — list of saints commemorated today.
///   3. Trailing `&Pretiosa` close.
///
/// Mirror of `martyrologium($lang)` lines 138-228.
pub fn martyrologium(_office: &OfficeOutput, _date: Date) -> Vec<RenderedLine> {
    // TODO(B18): port specials/specprima.pl:138-228.
    //
    // Data dependency: needs `data/martyrologium_latin.json` shipped
    // from `data/build_martyrologium_json.py` (~250 LOC of Python).
    // The build script walks `vendor/.../horas/Latin/Martyrologium/`
    // and emits one keyed-by-date JSON.
    unimplemented!("phase B18: martyrologium block")
}

/// Resolve the martyrology key for a given calendar date. Most days
/// map 1:1 to `MM-DD.txt`; movable feast days (Pascha, Pentecost
/// Octave) shift their keying. Mirror of `gregor()` calling pattern
/// in specprima.pl.
pub fn martyrology_key_for_date(_date: Date) -> String {
    // TODO(B18): port the date → key mapping. Trivial except for
    // moveable-feast supplements.
    unimplemented!("phase B18: martyrology_key_for_date")
}
