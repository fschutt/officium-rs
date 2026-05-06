//! Top-level office-hour orchestrator. Mirror of upstream
//! `vendor/divinum-officium/web/cgi-bin/horas/horas.pl::horas($hora)`
//! (lines 28-84) plus `getordinarium` (lines 579-602).
//!
//! Until M2 of the migration the working implementation lives in
//! [`crate::horas::compute_office_hour`]; this module re-exports it so
//! call sites can be written against `breviary::*` from the start.
//!
//! Post-migration this module becomes the entry point for `Office`
//! rendering; the existing `crate::horas` shim is preserved for a
//! release cycle then removed.

use crate::core::Rubric;
use crate::ordo::RenderedLine;

/// Re-export the working B7 entry point. Replaced by the full B20
/// orchestrator below (`compute_office_hour_full`) once the slices
/// land.
pub use crate::horas::{compute_office_hour, OfficeArgs};

/// The eight canonical hours of the Roman Office.
///
/// Strings used as section-key suffixes upstream (`Capitulum Tertia`,
/// `Hymnus Vespera`, `Ant Magnificat Vespera`). Tertia / Sexta / Nona
/// share the `Minor.txt` Ordinarium template — see
/// [`Hour::ordinarium_filename`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hour {
    Matutinum,
    Laudes,
    Prima,
    Tertia,
    Sexta,
    Nona,
    Vespera,
    Completorium,
}

impl Hour {
    /// All 8 hours in canonical liturgical order.
    pub const ALL: [Hour; 8] = [
        Hour::Matutinum,
        Hour::Laudes,
        Hour::Prima,
        Hour::Tertia,
        Hour::Sexta,
        Hour::Nona,
        Hour::Vespera,
        Hour::Completorium,
    ];

    /// The upstream string form (used as section-key suffix).
    pub const fn as_str(self) -> &'static str {
        match self {
            Hour::Matutinum    => "Matutinum",
            Hour::Laudes       => "Laudes",
            Hour::Prima        => "Prima",
            Hour::Tertia       => "Tertia",
            Hour::Sexta        => "Sexta",
            Hour::Nona         => "Nona",
            Hour::Vespera      => "Vespera",
            Hour::Completorium => "Completorium",
        }
    }

    /// Map to the underlying `Ordinarium/<name>.txt` file. Tertia /
    /// Sexta / Nona share `Minor.txt`; everything else is 1:1.
    pub const fn ordinarium_filename(self) -> &'static str {
        match self {
            Hour::Tertia | Hour::Sexta | Hour::Nona => "Minor",
            other => other.as_str(),
        }
    }

    /// Parse from the upstream string form. Accepts the Latin variants
    /// `Vesperae` (plural) → `Vespera` like the Perl entry.
    pub fn parse(s: &str) -> Option<Hour> {
        match s {
            "Matutinum"            => Some(Hour::Matutinum),
            "Laudes"               => Some(Hour::Laudes),
            "Prima"                => Some(Hour::Prima),
            "Tertia"               => Some(Hour::Tertia),
            "Sexta"                => Some(Hour::Sexta),
            "Nona"                 => Some(Hour::Nona),
            "Vespera" | "Vesperae" => Some(Hour::Vespera),
            "Completorium"         => Some(Hour::Completorium),
            _ => None,
        }
    }
}

/// Final B20 orchestrator — the breviary equivalent of
/// [`crate::ordo::render_mass`]. Drives the full per-hour pipeline:
/// resolve propers, walk the Ordinarium template, splice in psalmody /
/// hymn / capitulum / canticle / oratio / commemorations / suffrage,
/// run postprocess scrubs.
///
/// Replaces [`crate::horas::compute_office_hour`] when B20 lands;
/// until then this is `unimplemented!` and callers use the existing
/// shim (re-exported as [`compute_office_hour`]).
///
/// Mirrors `horas.pl::horas($hora)` lines 28-84.
pub fn compute_office_hour_full(
    _office: &crate::core::OfficeOutput,
    _hour: Hour,
    _corpus: &dyn crate::corpus::Corpus,
) -> Vec<RenderedLine> {
    // TODO(B20): port the full `horas.pl::horas($hora)` orchestrator.
    // Composes: getordinarium (B10 done) + specials walker (B10) +
    // per-hour helpers (B14-B19) + postprocess (B20).
    unimplemented!("phase B20: full hour orchestrator")
}

/// Convert an upstream `$hora` heading. Mirror of `horas.pl::adhoram`
/// (lines 18-24): `Vespera` → `Ad Vesperas`, `Tertia` → `Ad Tertiam`,
/// etc.
///
/// Used by the demo's headline; today the demo synthesises this in JS.
/// B13 moves it to Rust so the WASM API returns a fully-formatted
/// banner.
pub fn ad_horam_heading(_hour: Hour) -> String {
    // TODO(B13): port `horas.pl::adhoram` — appends the right Latin
    // suffix to the hour name. Trivial.
    unimplemented!("phase B13: ad_horam heading formatter")
}

/// Parse the active rubric and the `psalmvar` flag into the runtime
/// language toggle. Mirror of upstream `officium.pl:130-134` (Latin →
/// Latin-Bea swap when `psalmvar` is set).
///
/// Returns `(use_bea, lang_label)`.
pub fn resolve_psalter_variant(_rubric: Rubric, _psalmvar: bool) -> (bool, &'static str) {
    // TODO(B10): port officium.pl:130-134 psalmvar swap. Trivial.
    unimplemented!("phase B10: psalmvar Latin → Latin-Bea swap")
}
