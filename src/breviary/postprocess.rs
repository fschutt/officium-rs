//! Postprocess — text-level scrubs and wrappers.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/horas.pl`:
//!
//! - `resolve_refs($t, $lang)` (lines 89-212) — outer text walker.
//!   Splits the body into lines, expands `$<name>` and `&<name>` refs,
//!   applies the per-line "red prefix" / "large chapter" / "first
//!   letter initial" scrubs.
//! - `adjust_refs($name, $lang)` (lines 294-325) — rewrite a macro
//!   reference based on `$rule` (Requiem-gloria swap, Triduum
//!   gloria-omission, priest-vs-non-priest Dominus_vobiscum branch).
//! - `setlink($name, $ind, $lang)` (lines 329-440) — embed a link to
//!   a popup / expand-this-section action. UI-side; the Rust port
//!   emits structured `RenderedLine::Link` instead.
//! - `get_link_name($name)` (line 441) — translate a macro name to
//!   its display label, with rubric-conditional substitutions
//!   (`&Gloria1` → `&gloria` etc.).
//! - `setasterisk($line)` (lines 606-650) — psalm-verse asterisk
//!   placement (the breath-pause `*`).
//! - `getantcross($psalmline, $antline)` (lines 240-278) — Tridentine
//!   `‡` dagger marker on psalm verses that begin a new psalm
//!   subdivision.
//! - `depunct($item)` (line 280) — strip punctuation + de-accent
//!   for antiphon-vs-verse comparison.
//! - `columnsel($lang)` (line 652) — second-column language
//!   selection. Single-column always in the Rust port → identity
//!   helper.
//! - `postprocess_ant($ant, $lang)` (line 660) — antiphon-end period
//!   + Paschal Alleluia injection.
//! - `postprocess_vr($vr, $lang)` (line 680) — versicle/response
//!   Paschal Alleluia injection.
//! - `postprocess_short_resp($capit, $lang)` (line 697) — short
//!   responsory body Alleluia injection.
//! - `alleluia_required($dayname, $votive)` (line 729) — predicate.

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Top-level body-of-section walker. Splits a body into lines,
/// expands `$<name>` / `&<name>` refs, applies per-line scrubs.
///
/// Mirror of `resolve_refs($t, $lang)` lines 89-212.
pub fn resolve_refs(_office: &OfficeOutput, _body: &str) -> Vec<RenderedLine> {
    // TODO(B20): port horas.pl:89-212.
    unimplemented!("phase B20: resolve_refs")
}

/// Rewrite a macro reference based on the active rule body.
///
/// Mirror of `adjust_refs($name, $lang)` lines 294-325. Specific
/// rewrites:
///   - `&Gloria` + `Requiem gloria` rule → `$Requiem`
///   - `&Gloria` + Triduum → `Gloria omittitur` rubric line
///   - `&Dominus_vobiscum1` + non-priest + Preces Dominicales →
///     `prayer('Dominus')` line 4 ("Domine, exaudi orationem meam")
///   - `&Dominus_vobiscum2` + non-priest → same as above
pub fn adjust_refs(_office: &OfficeOutput, _name: &str) -> String {
    // TODO(B20): port horas.pl:294-325.
    unimplemented!("phase B20: adjust_refs")
}

/// Insert the breath-pause asterisk into a psalm verse.
///
/// Mirror of `setasterisk($line)` lines 606-650. The decision is
/// length-based: short verses get a single `*` at midpoint, long
/// verses split at the first comma after a syllable threshold.
pub fn set_asterisk(_line: &str) -> String {
    // TODO(B20): port horas.pl:606-650 (~45 LOC).
    unimplemented!("phase B20: set_asterisk")
}

/// Insert the Tridentine `‡` dagger marker on antiphon-matching
/// verses. Returns the modified psalm line. Mirror of `getantcross`
/// lines 240-278.
pub fn get_ant_cross(_psalm_line: &str, _ant_line: &str) -> String {
    // TODO(B20): port horas.pl:240-278.
    unimplemented!("phase B20: get_ant_cross")
}

/// Strip punctuation + diacritics for antiphon/verse comparison.
/// Mirror of `depunct($item)` line 280.
pub fn depunct(s: &str) -> String {
    // TODO(B20): port horas.pl:280-292. Trivial.
    let _ = s;
    unimplemented!("phase B20: depunct")
}

/// Postprocess one antiphon body. Mirror of `postprocess_ant`
/// line 660.
pub fn postprocess_ant(_office: &OfficeOutput, _ant: &mut String) {
    // TODO(B20): port horas.pl:660-676.
    // Two scrubs:
    //   1. Append a period if the antiphon doesn't end in one.
    //   2. Inject a single Paschal Alleluia under
    //      `alleluia_required && lang != gabc`.
    unimplemented!("phase B20: postprocess_ant")
}

/// Postprocess a versicle/response pair. Mirror of `postprocess_vr`
/// line 680.
pub fn postprocess_vr(_office: &OfficeOutput, _vr: &mut String) {
    // TODO(B20): port horas.pl:680-694.
    unimplemented!("phase B20: postprocess_vr")
}

/// Postprocess a short responsory body. Mirror of
/// `postprocess_short_resp` line 697.
pub fn postprocess_short_resp(_office: &OfficeOutput, _capit: &mut Vec<RenderedLine>) {
    // TODO(B20): port horas.pl:697-728.
    unimplemented!("phase B20: postprocess_short_resp")
}

/// Predicate: should the active office add a Paschal Alleluia to
/// antiphons / versicles / short responsories?
///
/// Mirror of `alleluia_required($dayname, $votive)` line 729-734.
///
/// Returns true when:
///   - season is Easter ("Pasc"), AND
///   - votive is not Office-of-the-Dead (C9) or BMV-Parva (C12).
pub fn alleluia_required(_office: &OfficeOutput) -> bool {
    // TODO(B20): port horas.pl:729-734. Trivial.
    unimplemented!("phase B20: alleluia_required")
}

/// Inject a single "alleluia" at the end of a body if it isn't
/// already present. Mirror of upstream `LanguageTextTools::ensure_single_alleluia`.
pub fn ensure_single_alleluia(_body: &mut String) {
    // TODO(B20): port LanguageTextTools::ensure_single_alleluia.
    // Mass-side may already have an equivalent; reuse if so.
    unimplemented!("phase B20: ensure_single_alleluia")
}

/// Inject a double "alleluia, alleluia" at the end of a body if it
/// isn't already present. Mirror of upstream
/// `LanguageTextTools::ensure_double_alleluia`.
pub fn ensure_double_alleluia(_body: &mut String) {
    // TODO(B20): port LanguageTextTools::ensure_double_alleluia.
    unimplemented!("phase B20: ensure_double_alleluia")
}
