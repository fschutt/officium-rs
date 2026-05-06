//! Gospel canticles — Benedictus, Magnificat, Nunc dimittis.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/horas.pl`:
//!
//! - `canticum($item, $lang)` (line 508) — top-level renderer for the
//!   gospel-canticle slot at Lauds (Benedictus, canticum 230),
//!   Vespers (Magnificat, canticum 231), Compline (Nunc dimittis,
//!   canticum 232).
//! - `ant123_special($lang)` (line 472) — overrides the canticle
//!   antiphon under three special cases:
//!     1. Advent O-antiphons (Dec 17–23 — `O Sapientia` etc.).
//!     2. Confessor-Pope second-Vespers Magnificat antiphon (`Dum
//!        esset Petrus in vinculis`).
//!     3. Compline's Nunc dimittis under the Praedicatorum rubric
//!        (special Quad antiphon).
//! - `final_marian_antiphon` — separate from `canticum`, but lives
//!   here too because it's the closing antiphon at Compline (Alma
//!   Redemptoris / Ave Regina caelorum / Regina caeli / Salve Regina).
//!   Mirror of `specials.pl:313-340` "Antiphona finalis BMV".

use crate::breviary::horas::Hour;
use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Render the gospel canticle slot for an hour.
///
/// Hour-to-canticle mapping:
///   * `Laudes` → Benedictus (canticum #230)
///   * `Vespera` → Magnificat (canticum #231)
///   * `Completorium` → Nunc dimittis (canticum #232)
///
/// Mirror of `canticum($item, $lang)` lines 508-569.
pub fn canticum(_office: &OfficeOutput, _hour: Hour) -> Vec<RenderedLine> {
    // TODO(B16): port horas.pl:508-569 (~60 LOC).
    // Composition:
    //   1. Resolve the antiphon (via ant123_special if applicable,
    //      else via getantvers).
    //   2. Lookup the canticle body — psalm number 230/231/232 in the
    //      psalm corpus (`crate::breviary::corpus::psalm`).
    //   3. Format via crate::breviary::antetpsalm::format_antiphon_psalm.
    //   4. Apply special second antiphon if Praedicatorum + Quad3.
    unimplemented!("phase B16: canticum renderer")
}

/// Special antiphon overrides for the gospel canticle. Three cases:
///
/// 1. **Advent O-antiphons** (Dec 17-23, winner is Tempora):
///    Magnificat-of-Vespera takes the day-keyed `Adv Ant 17`–
///    `Adv Ant 23` from `Psalterium/Special/Major Special.txt`. On
///    Dec 21 and 23 the Lauds Benedictus also takes the L-suffix
///    variant (`Adv Ant 21L`).
///
/// 2. **Confessor-Pope second Vespers**: Magnificat antiphon is
///    `Dum esset Petrus in vinculis`. Mirror of
///    [`crate::breviary::papal::papal_antiphon_dum_esset`].
///
/// 3. **Praedicatorum Quad3**: a special Compline Nunc dimittis
///    antiphon set with two-line form.
///
/// Returns `(antiphon, duplex_flag)` when one of these fires; `None`
/// when the default antiphon applies.
///
/// Mirror of `ant123_special` lines 472-503.
pub fn ant123_special(
    _office: &OfficeOutput,
    _hour: Hour,
) -> Option<(String, bool)> {
    // TODO(B16): port horas.pl:472-503.
    unimplemented!("phase B16: ant123_special")
}

/// Closing Marian antiphon at Compline. One of four, keyed by season:
///
/// | Season | Antiphon |
/// |---|---|
/// | Advent / Christmas | `Alma Redemptoris Mater` |
/// | Septuagesima / Lent (until Holy Saturday) | `Ave Regina caelorum` |
/// | Easter (Pasc1-0 → Pasc7-6) | `Regina caeli laetare` |
/// | Default | `Salve Regina` |
///
/// Cistercian uses Salve Regina year-round.
///
/// Mirror of `specials.pl:313-340` "Antiphona finalis BMV".
pub fn final_marian_antiphon(_office: &OfficeOutput) -> RenderedLine {
    // TODO(B16): port specials.pl:313-340. Trivial season check.
    unimplemented!("phase B16: final_marian_antiphon")
}
