//! Papal commemorations and rule helpers.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/horascommon.pl`:
//!
//! - `papal_commem_rule($)` (line 2175) — extract the papal rank /
//!   class / "tu es Petrus" flag from a winner's `[Rule]` body.
//! - `papal_rule($%)` (line 2189) — same with extra context for the
//!   commemorations branch.
//! - `papal_prayer($$$$;$)` (line 2200) — emit the papal-prayer body
//!   inserted at Lauds and Vespers under pre-1960 rubric.
//! - `papal_antiphon_dum_esset($)` (line 2233) — special Magnificat
//!   antiphon for Confessor-Popes at second Vespers.
//!
//! These appear at the Common of Supreme Pontiffs and at any office
//! that carries the `papal` token in its `[Rule]` body. Pre-1960 only;
//! 1960+ scrubbed papal commemorations.

use crate::core::OfficeOutput;

/// Class of papal subject — Pope (P), Confessor-Pope (C), Martyr-Pope (M),
/// Doctor-Pope (D). Drives which papal-prayer body and which Magnificat
/// antiphon are used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PapalClass {
    /// Pope (default — used for office of a pope).
    Pope,
    /// Confessor and Pope (most common — `tu es Petrus`).
    Confessor,
    /// Martyr and Pope (St Peter, St Clement, etc.).
    Martyr,
    /// Doctor and Pope (St Gregory the Great, etc.).
    Doctor,
}

/// Parse a winner's `[Rule]` body for papal designation. Mirror of
/// `papal_commem_rule` (line 2175). Returns `(rank_token, class,
/// is_te_es_petrus)` from the rule body's `papal=…` directive.
///
/// Returns `None` when the rule body has no `papal=…` directive.
pub fn papal_rule_from_rule_body(_rule: &str) -> Option<(String, PapalClass, bool)> {
    // TODO(B14): port horascommon.pl:2175-2199. Parses
    //   `papal=<rank>;<class>;<flag>`
    // out of the rule body — the same shape as `Suffr=…` etc.
    unimplemented!("phase B14: papal_rule_from_rule_body")
}

/// Emit the papal-prayer body inserted at Lauds / Vespers when the
/// office is of a pope. Pre-1960 only.
///
/// Mirror of `papal_prayer` (line 2200).
pub fn papal_prayer_body(
    _office: &OfficeOutput,
    _class: PapalClass,
    _section: &str, // "Oratio" / "Sequentia" / etc.
) -> Option<String> {
    // TODO(B14): port horascommon.pl:2200-2232.
    unimplemented!("phase B14: papal_prayer_body")
}

/// Special Magnificat antiphon `Dum esset Petrus in vinculis` for
/// Confessor-Popes at second Vespers. Mirror of
/// `papal_antiphon_dum_esset` (line 2233).
pub fn papal_antiphon_dum_esset(_office: &OfficeOutput) -> Option<String> {
    // TODO(B14): port horascommon.pl:2233-2242. Used by
    // `crate::breviary::canticum::ant123_special` when the winner
    // is Sancti, vespera == 3, version != Tridentine, papal class C.
    unimplemented!("phase B14: papal_antiphon_dum_esset")
}
