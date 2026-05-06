//! Antiphon + psalm pairing — formats one psalmody verse group.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials/psalmi.pl::antetpsalm`
//! (line 637). Composes:
//!
//! 1. The antiphon (intoned form before, full form after).
//! 2. The psalm body (looked up from
//!    `Psalterium/Psalmorum/Psalm{N}.txt`).
//! 3. The Gloria Patri suffix (suppressed under Triduum / Defunctorum).
//!
//! Output is a sequence of `RenderedLine`s — `Plain` for psalm verses,
//! `Spoken { role: "Ant" }` for the antiphon, with `Macro` for the
//! Gloria Patri.
//!
//! ## Asterisk insertion
//!
//! Each psalm verse carries an asterisk at the natural breath-pause.
//! The Perl helper `setasterisk` (`horas.pl:606-650`) decides where —
//! by syllable count + comma + length thresholds. Lives in
//! [`crate::breviary::postprocess::set_asterisk`] post-B20; called
//! from this module's verse-formatting helper.

use crate::breviary::psalter::PsalmRendered;
use crate::ordo::RenderedLine;

/// Render one antiphon-psalm group as a sequence of `RenderedLine`s.
///
/// Input:
///   * `psalm` — pre-resolved antiphon + psalm number (body optional;
///     when missing, fetches from the corpus).
///   * `duplex_flag` — when true, antiphon is sung in full before AND
///     after the psalm (festal mode); when false, intoned before and
///     full after (ferial mode).
///
/// Output:
///   * `Section { label: "Ant {N}" }`
///   * `Plain { body: <antiphon> }` — intoned form
///   * `Plain { body: <psalm verse 1> }` … (with `*` asterisk inserted)
///   * `Plain { body: "Gloria Patri…" }` — when `gloria_patri == true`
///   * `Plain { body: <antiphon> }` — full form (closes the antiphon)
///
/// Mirror of `antetpsalm(\@psalmi, $duplexf, $lang)` lines 637-684.
pub fn format_antiphon_psalm(_psalm: &PsalmRendered) -> Vec<RenderedLine> {
    // TODO(B15): port specials/psalmi.pl:637-684 (~50 LOC).
    // Split into:
    //   1. Lookup psalm body if absent (crate::breviary::corpus::psalm)
    //   2. Format antiphon intoned vs full
    //   3. Apply set_asterisk to each verse
    //   4. Append Gloria Patri / Doxology when flag set
    //   5. Append antiphon full at end
    unimplemented!("phase B15: format_antiphon_psalm")
}

/// Format the Gloria Patri (Doxology) suffix that closes a psalm
/// group. Per-rubric — pre-1955 carries a longer wording on certain
/// feasts.
pub fn gloria_patri(_office: &crate::core::OfficeOutput) -> RenderedLine {
    // TODO(B15): port the trailing Gloria Patri block. Today this is
    // a `&Gloria` macro reference resolved against `Psalterium/
    // Common/Prayers`.
    unimplemented!("phase B15: gloria_patri")
}
