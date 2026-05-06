//! Hymn selection per hour.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials/hymni.pl`:
//!
//! - `gethymn($lang)` (line 5) — top-level entry. Reads
//!   `Hymnus $hora` from the day file (with tempora / commune
//!   fallbacks) and applies the rubric-conditional doxology selector.
//! - `hymnusmajor($lang)` (line 67) — Lauds / Vespera-specific shaping
//!   (Iste Confessor / Exsultet caelum etc. with rubric-keyed
//!   strophe selection).
//! - `doxology($lang)` (line 125) — picks the doxology strophe based
//!   on `dayname[0]` (season), `version` (rubric), `winner{Rule}` and
//!   `commune`. Four-state lookup; 1962 strips doxologies entirely.
//!
//! Hymns per hour (Roman):
//!
//! | Hour | Hymn |
//! |---|---|
//! | Matutinum | season-keyed; `Aeterne rerum conditor` (Lauds-of-Sunday Sundays in some old uses) |
//! | Laudes | season-keyed; festal `Iste Confessor`, ferial `Splendor paternae gloriae` |
//! | Prima | `Jam lucis orto sidere` (always) |
//! | Tertia | `Nunc Sancte nobis Spiritus` (always) |
//! | Sexta | `Rector potens, verax Deus` (always) |
//! | Nona | `Rerum, Deus, tenax vigor` (always) |
//! | Vespera | season-keyed |
//! | Completorium | `Te lucis ante terminum` (always) |

use crate::breviary::horas::Hour;
use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Top-level hymn dispatcher. Mirrors `gethymn($lang)` line 5.
pub fn get_hymn(_office: &OfficeOutput, _hour: Hour) -> Vec<RenderedLine> {
    // TODO(B16): port specials/hymni.pl:5-66.
    // Composition:
    //   1. Lookup `Hymnus $hora` via crate::breviary::proprium
    //   2. If empty, fall through to commune / tempora / psalterium-special
    //   3. Apply rubric-keyed doxology via `doxology()`
    //   4. Strip mute-vowel italicization markers (`[æ]`, `[i]`)
    //      → handled at postprocess stage
    unimplemented!("phase B16: get_hymn")
}

/// Lauds + Vespers-specific hymn selection. Some hymns split per-
/// rubric (Iste Confessor has a 1568 form and a 1602 form, and the
/// later form is strophe-rearranged after Pius X).
///
/// Mirror of `hymnusmajor` (line 67).
pub fn hymnus_major(_office: &OfficeOutput, _hour: Hour) -> Vec<RenderedLine> {
    // TODO(B16): port specials/hymni.pl:67-124.
    unimplemented!("phase B16: hymnus_major")
}

/// Doxology strophe selector. Picks one of:
///
/// - `Jesu tibi sit gloria` (Christmas / Epiphany / Ascension)
/// - `Deo Patri sit gloria, Et Filio` (default)
/// - `Glória tibi, Dómine, Qui natus es` (Christmas variant)
/// - `Glória tibi, Dómine, Qui surrexísti` (Easter)
/// - `Sit laus Patri, sit Filio, … Spiritui` (Pentecost)
/// - `Tibi laus, perennis gloria` (Trinity)
/// - empty (R60 strips doxologies entirely)
///
/// Mirror of `doxology` (line 125).
pub fn doxology(_office: &OfficeOutput) -> Option<&'static str> {
    // TODO(B16): port specials/hymni.pl:125-162. The ~40 LOC of
    // Perl is a series of `elsif` season + version checks.
    unimplemented!("phase B16: doxology selector")
}
