//! Prime-specific blocks.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials/specprima.pl`:
//!
//! - `lectio_brevis_prima($lang)` (line 5) — Prime's short reading,
//!   with rubric-conditional source selection.
//! - `capitulum_prima($lang, $has_responsorium)` (line 55) — Prime
//!   Capitulum + (optionally) Responsorium.
//! - `get_prima_responsory($lang)` (line 110) — short responsory for
//!   Prime under monastic / Cistercian rubrics.
//! - `martyrologium($lang)` (line 138) — full martyrology block for
//!   the day. See [`crate::breviary::martyrologium`].
//! - `luna($d, $m, $y)` (line 229) — golden number / lunar age
//!   computation.
//! - `gregor($d, $m, $y)` (line 277) — Gregorian-leap-shifted
//!   martyrology date.
//!
//! Prime is by far the densest hour after Matins because of the
//! Martyrologium and the optional `De Officio Capituli` block (1960
//! only) and the Regula reading (Monastic only).

use crate::core::{Date, OfficeOutput};
use crate::ordo::RenderedLine;

/// Render the Lectio brevis at Prime.
///
/// Mirror of `lectio_brevis_prima` lines 5-54.
pub fn lectio_brevis_prima(_office: &OfficeOutput) -> Vec<RenderedLine> {
    // TODO(B18): port specials/specprima.pl:5-54.
    // Source selection cascade:
    //   1. `Lectio Prima` from the day file (winner).
    //   2. Commune fallback.
    //   3. `Psalterium/Special/Prima Special.txt` keyed by season
    //      (`Lectio Prima Adv`, `Lectio Prima Quad`, etc.).
    //   4. Default `Lectio Prima` from psalterium-special.
    unimplemented!("phase B18: lectio_brevis_prima")
}

/// Render the Capitulum at Prime, with optional short responsory.
///
/// Mirror of `capitulum_prima($lang, $has_responsorium)` lines 55-109.
pub fn capitulum_prima(
    _office: &OfficeOutput,
    _has_responsorium: bool,
) -> Vec<RenderedLine> {
    // TODO(B18): port specials/specprima.pl:55-109.
    unimplemented!("phase B18: capitulum_prima")
}

/// Render the short responsory at Prime under Monastic / Cistercian
/// rubrics. Returns empty under Roman rubrics.
///
/// Mirror of `get_prima_responsory($lang)` lines 110-137.
pub fn get_prima_responsory(_office: &OfficeOutput) -> Vec<RenderedLine> {
    // TODO(B18): port specials/specprima.pl:110-137. Roman parity
    // pass returns empty.
    unimplemented!("phase B18: get_prima_responsory")
}

/// Compute the lunar age (golden number) for a Gregorian date.
///
/// Mirror of `luna($d, $m, $y)` lines 229-276. Used to format the
/// Martyrologium's "Luna {N}" line.
pub fn luna(_date: Date) -> u8 {
    // TODO(B18): port specials/specprima.pl:229-276. Pure arithmetic
    // over Gregorian date — no corpus dependency.
    unimplemented!("phase B18: luna (golden number)")
}

/// Apply the paschal-shift to a Gregorian date for martyrology
/// lookup. The Martyrologium tables are keyed by the *liturgical*
/// March 25 = "annunciation kalends" reckoning, not by the civil
/// calendar.
///
/// Mirror of `gregor($d, $m, $y)` lines 277-366. DiPippo's tables are
/// the authoritative reference when the Perl computation diverges.
pub fn gregor(_date: Date) -> Date {
    // TODO(B18): port specials/specprima.pl:277-366. Defer to
    // DiPippo if the Perl algorithm produces a different answer.
    unimplemented!("phase B18: gregor (martyrology date shift)")
}
