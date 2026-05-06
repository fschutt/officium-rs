//! Per-day office banner — `setheadline` and `rankname`.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/horascommon.pl`:
//!
//! - `setheadline` (lines 1868-1884) — assembles the formatted
//!   headline string ("Feria Quinta in Cena Domini ~ I. classis ~ …")
//!   that the upstream UI shows above the office.
//! - `rankname` (lines 1885-1990) — translates the numeric rank into
//!   the per-rubric Latin label ("Duplex II classis", "Semiduplex",
//!   "I. classis", "III. classis", …).
//!
//! Today the demo synthesises the headline in JS. B13 moves it to
//! Rust so the WASM API returns a fully-formatted banner and the
//! per-rubric label conventions don't drift.

use crate::core::{OfficeOutput, Rank, Rubric};

/// Build the display headline for an office output.
///
/// Examples:
/// - `Tempora/Pasc1-0` (1570): "Dominica Resurrectionis ~ Duplex I. classis ~ Tempora Paschalis ~ Albus"
/// - `Sancti/05-04` (1960): "S. Monicae Viduae ~ III. classis ~ Tempore Paschali ~ Albus"
///
/// Mirrors `setheadline` lines 1868-1884.
pub fn build_headline(_output: &OfficeOutput) -> String {
    // TODO(B13): port horascommon.pl:1868-1884.
    // Composition: `<title> ~ <rank-label> ~ <season-label> ~ <color-label>`
    // where each segment is per-rubric.
    unimplemented!("phase B13: build_headline")
}

/// Translate a rank into its per-rubric Latin label.
/// Mirror of `rankname` lines 1885-1990.
///
/// Examples (all from the same Rank::DuplexIIClassis):
/// - 1570: "Duplex II. classis"
/// - 1955: "II classis"
/// - 1960: "II. classis"
pub fn rank_name(_rank: &Rank, _rubric: Rubric) -> &'static str {
    // TODO(B13): port horascommon.pl:1885-1990 (~100 LOC of Perl).
    // Mostly a lookup table per (rank-num, rubric) tuple with a
    // handful of edge cases (Vigilia majoris, Octava III ordinis).
    unimplemented!("phase B13: rank_name lookup")
}

/// Format the season segment of the headline. Per-rubric — pre-1955
/// uses "Tempore N. Paschae", 1960 uses "Tempore Paschali".
pub fn season_label(_output: &OfficeOutput) -> &'static str {
    // TODO(B13): port the per-rubric season-label table.
    unimplemented!("phase B13: season_label")
}
