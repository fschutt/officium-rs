//! Oratio (collect) + commemorations — the densest single chunk.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specials/orationes.pl`
//! (1215 LOC of Perl):
//!
//! - `oratio($lang, $month, $day, %params)` (line 17) — top-level
//!   entry. Composes:
//!     1. Day's collect (`Oratio` from the day file).
//!     2. Commemoration of any concurring office (FromVespers /
//!        SecondVespers branches).
//!     3. Octave commemoration (per `[Rule]` `Octava Christi` etc.).
//!     4. Vigil commemoration.
//!     5. Sunday commemoration (when a feast displaces a Sunday).
//!     6. All-Saints / All-Souls Suffragium (pre-1955 only).
//!     7. `[Rule]` "Add Defunctorum" at Vespera under the Conventual
//!        Mass-of-the-Dead anniversary days.
//!
//! - `checkcommemoratio` (line 7) — predicate: should this office
//!   carry a commemoration of `$commemoratio`?
//! - `delconclusio` (line 623) — strip the `Per Dominum / Qui vivis`
//!   conclusion from a collect when chaining commemorations (the
//!   conclusion is only emitted on the final collect).
//! - `getcommemoratio($commemorabile, $lang)` (line 644) — pull the
//!   `Oratio` (and antiphon + versicle if at Lauds / Vespera) for one
//!   commemorated office.
//! - `vigilia_commemoratio($lang)` (line 823) — emit the Vigil
//!   commemoration for tomorrow's I-class feast.
//! - `getsuffragium($lang)` (line 879) — Suffragium of All Saints
//!   body. Pre-1955 only. See [`crate::breviary::suffragium`].
//! - `getrefs($body)` (line 942) — chase `@`-references inside an
//!   oratio body. (Today partially in `crate::horas::expand_at_redirect`.)
//! - `oratio_solemnis($lang)` (line 1134) — Triduum-only solemn
//!   collect form.

use crate::breviary::horas::Hour;
use crate::core::{Date, OfficeOutput};
use crate::ordo::RenderedLine;

/// Optional parameters for [`oratio`]. Mirrors the `%params` hash the
/// Perl entry accepts (`special` flag for Triduum / `loadspecial`
/// branch).
#[derive(Debug, Clone, Default)]
pub struct OratioParams {
    /// Set when called from the Triduum special-conclusion path —
    /// suppresses the standard `Per Dominum` conclusion.
    pub special: bool,
}

/// Top-level oratio renderer. Drives the full
/// collect-plus-commemorations chain.
///
/// Mirror of `oratio($lang, $month, $day, %params)` lines 17-622.
pub fn oratio(
    _office: &OfficeOutput,
    _hour: Hour,
    _date: Date,
    _params: OratioParams,
) -> Vec<RenderedLine> {
    // TODO(B17): port specials/orationes.pl:17-622 (~600 LOC).
    // The richest dispatcher in the breviary leg. Fans out to:
    //   - `getcommemoratio` for each surviving commemoration
    //   - `vigilia_commemoratio` for tomorrow's vigil
    //   - `getsuffragium` for Suffragium of All Saints (pre-1955)
    //   - `oratio_solemnis` for Triduum special form
    //   - `delconclusio` to strip conclusions on chained collects
    unimplemented!("phase B17: oratio entry")
}

/// Predicate: should this office carry a commemoration of the
/// `commemoratio` slot? Mirror of `checkcommemoratio` line 7.
///
/// Drops the commemoration when:
///   - The office is already of the same rank or higher.
///   - The Octave-of-Christmas / -Epiphany / -Easter rules suppress.
///   - Under 1960, simple commemorations are entirely abolished.
pub fn check_commemoratio(_office: &OfficeOutput) -> bool {
    // TODO(B17): port specials/orationes.pl:7-16.
    unimplemented!("phase B17: check_commemoratio")
}

/// Strip the `Per Dominum nostrum…` conclusion from a collect body.
/// Used when chaining commemorations — the conclusion is emitted only
/// on the FINAL collect of a chain.
///
/// Mirror of `delconclusio` line 623.
pub fn del_conclusio(_body: &str) -> String {
    // TODO(B17): port specials/orationes.pl:623-643.
    unimplemented!("phase B17: del_conclusio")
}

/// Pull the oratio + (optionally) antiphon + versicle for one
/// commemorated office. At Lauds and Vespers, commemorations carry
/// not just the collect but the gospel-canticle antiphon and
/// versicle of the commemorated office.
///
/// Mirror of `getcommemoratio($commemorabile, $lang)` line 644.
pub fn get_commemoratio(
    _office: &OfficeOutput,
    _hour: Hour,
    _commemoratio_key: &crate::core::FileKey,
) -> Vec<RenderedLine> {
    // TODO(B17): port specials/orationes.pl:644-822.
    unimplemented!("phase B17: get_commemoratio")
}

/// Emit a Vigil commemoration. When tomorrow's office is a I-class
/// feast with a Vigil ranked Semiduplex (or Simplex post-1955), today
/// commemorates that Vigil at second Vespers / Lauds.
///
/// Mirror of `vigilia_commemoratio($lang)` line 823.
pub fn vigilia_commemoratio(
    _today: &OfficeOutput,
    _tomorrow: &OfficeOutput,
    _hour: Hour,
) -> Vec<RenderedLine> {
    // TODO(B17): port specials/orationes.pl:823-878.
    unimplemented!("phase B17: vigilia_commemoratio")
}

/// Chase `@`-references inside an oratio body. Mirror of `getrefs`
/// line 942. Today the 1-hop variant is in
/// `crate::horas::expand_at_redirect`; the full multi-hop logic with
/// `:s/PAT/REPL/` substitutions ports here.
pub fn get_refs(_body: &str) -> String {
    // TODO(B17): port specials/orationes.pl:942-1133.
    unimplemented!("phase B17: get_refs (multi-hop @-redirect)")
}

/// Solemn collect form for the Triduum. The Holy Thursday / Good
/// Friday / Holy Saturday collects use the `oratio_solemnis` shape:
/// long invocation (`Oremus, Reverendissimi`) + collect body +
/// shortened conclusion.
///
/// Mirror of `oratio_solemnis($lang)` line 1134.
pub fn oratio_solemnis(
    _office: &OfficeOutput,
    _hour: Hour,
) -> Vec<RenderedLine> {
    // TODO(B17): port specials/orationes.pl:1134-1215.
    unimplemented!("phase B17: oratio_solemnis")
}
