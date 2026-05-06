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
/// (lines 18-24).
///
/// Perl logic (verbatim port):
///
/// ```perl
/// my $head = "Ad $hora";
/// $head =~ s/a$/am/;
/// $head = 'Ad Vesperas' if $hora =~ /vesper/i;
/// ```
///
/// In Roman declension this gives the accusative-of-motion form: "Ad
/// Tertiam", "Ad Sextam", "Ad Nonam", "Ad Primam", with the special
/// "Ad Vesperas" (plural) for Vespera and the unchanged forms "Ad
/// Matutinum" / "Ad Laudes" / "Ad Completorium" (already accusative
/// or non-`a`-ending).
pub fn ad_horam_heading(hour: Hour) -> String {
    if matches!(hour, Hour::Vespera) {
        return "Ad Vesperas".to_string();
    }
    let name = hour.as_str();
    if name.ends_with('a') {
        // Perl `s/a$/am/` — replace trailing `a` with `am`.
        format!("Ad {}m", name)
    } else {
        format!("Ad {name}")
    }
}

/// Resolve the active psalter language for a Latin-locale renderer.
///
/// Mirror of upstream `officium.pl:130-134`:
///
/// ```perl
/// if ($psalmvar) {
///   $lang1 = 'Latin-Bea' if $lang1 eq 'Latin' && $lang2 ne 'Latin-Bea';
///   $lang2 = 'Latin-Bea' if $lang2 eq 'Latin' && $lang1 ne 'Latin-Bea';
/// }
/// ```
///
/// The Rust port is single-column (no `lang2`) so the rule
/// degenerates to: when `psalmvar` is set, swap `Latin` → `Latin-Bea`.
/// Other Latin-derived locales (none currently exist) would pass
/// through unchanged.
///
/// `_rubric` is reserved — the upstream Perl never reads `$version`
/// here, but the parameter is kept on the signature so future
/// rubric-keyed psalter variants (none defined yet) don't break the
/// API.
///
/// Returns `(use_bea, lang_label)` — `use_bea` is what callers pass
/// to [`crate::breviary::corpus::psalm`] for the body lookup;
/// `lang_label` is the canonical language label for headline /
/// trace output.
pub fn resolve_psalter_variant(_rubric: Rubric, psalmvar: bool) -> (bool, &'static str) {
    if psalmvar {
        (true, "Latin-Bea")
    } else {
        (false, "Latin")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hour_as_str_round_trip() {
        for h in Hour::ALL {
            assert_eq!(Hour::parse(h.as_str()), Some(h), "round-trip failed for {h:?}");
        }
    }

    #[test]
    fn hour_parse_accepts_vesperae_alias() {
        // Upstream officium.pl:30 normalises `Vesperae` (plural) →
        // `Vespera` before dispatch.
        assert_eq!(Hour::parse("Vesperae"), Some(Hour::Vespera));
        assert_eq!(Hour::parse("Vespera"), Some(Hour::Vespera));
    }

    #[test]
    fn hour_parse_rejects_unknown() {
        assert_eq!(Hour::parse("NotAnHour"), None);
        assert_eq!(Hour::parse(""), None);
        // Case-sensitive — Perl is case-sensitive too at this layer.
        assert_eq!(Hour::parse("vespera"), None);
    }

    #[test]
    fn ordinarium_filename_collapses_minor_hours() {
        assert_eq!(Hour::Tertia.ordinarium_filename(), "Minor");
        assert_eq!(Hour::Sexta.ordinarium_filename(), "Minor");
        assert_eq!(Hour::Nona.ordinarium_filename(), "Minor");
        // Non-minor hours are 1:1.
        assert_eq!(Hour::Matutinum.ordinarium_filename(), "Matutinum");
        assert_eq!(Hour::Laudes.ordinarium_filename(), "Laudes");
        assert_eq!(Hour::Prima.ordinarium_filename(), "Prima");
        assert_eq!(Hour::Vespera.ordinarium_filename(), "Vespera");
        assert_eq!(Hour::Completorium.ordinarium_filename(), "Completorium");
    }

    #[test]
    fn ad_horam_heading_matches_perl_adhoram() {
        // Pinned against Perl `adhoram` output for every hour.
        assert_eq!(ad_horam_heading(Hour::Matutinum),    "Ad Matutinum");
        assert_eq!(ad_horam_heading(Hour::Laudes),       "Ad Laudes");
        assert_eq!(ad_horam_heading(Hour::Prima),        "Ad Primam");
        assert_eq!(ad_horam_heading(Hour::Tertia),       "Ad Tertiam");
        assert_eq!(ad_horam_heading(Hour::Sexta),        "Ad Sextam");
        assert_eq!(ad_horam_heading(Hour::Nona),         "Ad Nonam");
        assert_eq!(ad_horam_heading(Hour::Vespera),      "Ad Vesperas");
        assert_eq!(ad_horam_heading(Hour::Completorium), "Ad Completorium");
    }

    #[test]
    fn resolve_psalter_variant_swaps_under_psalmvar() {
        // psalmvar off — Vulgate text (false), `Latin` label.
        assert_eq!(
            resolve_psalter_variant(Rubric::Tridentine1570, false),
            (false, "Latin"),
        );
        assert_eq!(
            resolve_psalter_variant(Rubric::Rubrics1960, false),
            (false, "Latin"),
        );
        // psalmvar on — Bea text (true), `Latin-Bea` label.
        assert_eq!(
            resolve_psalter_variant(Rubric::Tridentine1570, true),
            (true, "Latin-Bea"),
        );
        assert_eq!(
            resolve_psalter_variant(Rubric::Rubrics1960, true),
            (true, "Latin-Bea"),
        );
    }

    #[test]
    fn hour_all_is_canonical_order() {
        // Liturgical day order — Matutinum first, Completorium last.
        let names: Vec<&str> = Hour::ALL.iter().map(|h| h.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Matutinum",
                "Laudes",
                "Prima",
                "Tertia",
                "Sexta",
                "Nona",
                "Vespera",
                "Completorium",
            ],
        );
    }
}
