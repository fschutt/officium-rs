//! Suffragium of All Saints — pre-1955 only.
//!
//! Mirror of:
//! - `vendor/divinum-officium/web/cgi-bin/horas/specials.pl::checksuffragium`
//!   (lines 700-768) — predicate.
//! - `vendor/divinum-officium/web/cgi-bin/horas/specials/orationes.pl::getsuffragium`
//!   (lines 879-941) — body emission.
//!
//! The Suffragium is a fixed commemorative block at Lauds / Vespers
//! invoking BVM, Apostles, and the patron saint of the church (or a
//! locally-determined cycle). Pius XII's 1955 reform abolished it on
//! all but a handful of days; 1960 abolished it entirely. So:
//!
//! - **Tridentine 1570 / 1910** — full Suffragium on most ferials.
//! - **Divino Afflatu 1911** — same as 1910.
//! - **Reduced 1955** — abolished except on day-1 ferials before
//!   Septuagesima.
//! - **Rubrics 1960** — never emitted.
//!
//! The body is one of three rotating cycles depending on day-of-week
//! and the per-office `Suffr=…` rule directive.

use crate::breviary::horas::Hour;
use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Predicate: should the active office carry a Suffragium block?
///
/// Returns false when:
///   - Active rubric is 1960 (always suppressed).
///   - Today is a I-class feast (Suffragium yields to feast).
///   - Active rubric is 1955 and today is not in the small "Sufr_55"
///     window (basically Christmas → Septuagesima Saturday).
///   - Today is in Quad5 (Passion week) — Suffragium suppressed
///     under 1570 too.
///   - Today is in Quad6 (Holy Week) — Suffragium always suppressed.
///
/// Mirror of `checksuffragium` (specials.pl:700-768).
pub fn check_suffragium(_office: &OfficeOutput, _hour: Hour) -> bool {
    // TODO(B12): port specials.pl:700-768 (~70 LOC of layered
    // rubric / season checks).
    unimplemented!("phase B12: check_suffragium")
}

/// Emit the Suffragium body. Three rotating cycles selected by the
/// `Suffr=…` rule directive on the day's `[Rule]` body and the
/// day-of-week.
///
/// Mirror of `getsuffragium($lang)` (orationes.pl:879-941).
pub fn get_suffragium_body(
    _office: &OfficeOutput,
    _hour: Hour,
) -> Vec<RenderedLine> {
    // TODO(B17): port specials/orationes.pl:879-941. Fetches one of
    // `Suffr Maria` / `Suffr Apost` / `Suffr Patrono` from the per-day
    // file (or `Psalterium/Special/Major Special.txt` when missing
    // from the day) and formats as antiphon + versicle + collect.
    unimplemented!("phase B17: get_suffragium_body")
}

/// Parse a `Suffr=…` directive from a `[Rule]` body. Returns an
/// ordered list of suffragium-slot keys (each rotating cycle is one
/// slot). The slot for today is `dayofweek % suffr_groups.len()`.
///
/// Example: `Suffr=Maria3;Ecclesiae,Papa;;` parses to
/// `[["Maria3"], ["Ecclesiae", "Papa"], []]`.
///
/// Mirror of the parsing block at orationes.pl:885-905.
pub fn parse_suffragium_rule(_rule: &str) -> Vec<Vec<String>> {
    // TODO(B12): port the rule-body parser. Used by `oratio()` and
    // by the Mass-side Suffragium in `crate::mass::parse_suffragium_rule`
    // (already exists for Mass — see if extracting to a shared
    // helper is cheaper than duplicating).
    unimplemented!("phase B12: parse_suffragium_rule")
}
