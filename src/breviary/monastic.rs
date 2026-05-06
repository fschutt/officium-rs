//! Pre-Trident Monastic cursus — input-config accepted, impl stubbed.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/monastic.pl`
//! (675 LOC). The Mass-side port doesn't ship Monastic either; the
//! Breviary first-parity pass mirrors that decision.
//!
//! Per `BREVIARY_PORT_PLAN.md §7.3`, the API surface is stable:
//! callers pass `OfficeInput { cursus: Cursus::Monastic, … }` and the
//! breviary entry points dispatch here. The Roman path is the only
//! one with a working body during the first parity pass — Monastic
//! calls land in `unimplemented!()` so the unintended fallback is
//! loud rather than silent.
//!
//! When Monastic support is added (post-B20, separate slice plan),
//! this file holds:
//!   - `Regula` — daily Rule-of-St-Benedict reading at Prime.
//!   - Monastic-specific psalmody overrides (read from
//!     `[Monastic Laudes]` / `[Monastic Vespera]` keys).
//!   - Monastic long responsory at Lauds / Vespera (referenced from
//!     [`crate::breviary::capitulum::monastic_major_responsory`]).

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Render the Regula reading at Monastic Prime.
///
/// Mirror of `monastic.pl::regula`.
pub fn regula(_office: &OfficeOutput) -> Vec<RenderedLine> {
    // TODO(post-B20): port `monastic.pl::regula`. The breviary
    // specials walker only routes here when `office.cursus ==
    // Cursus::Monastic`; first parity pass panics so the Monastic
    // input-config flag fires loudly.
    unimplemented!("Monastic cursus: out of scope for first parity pass — see BREVIARY_PORT_PLAN.md §7.3")
}
