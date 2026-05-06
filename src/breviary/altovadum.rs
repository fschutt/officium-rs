//! Cistercian "Altovadensis" cursus — input-config accepted, impl stubbed.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/altovadum.pl`
//! (1146 LOC). The Mass-side port doesn't ship Cistercian either.
//!
//! Per `BREVIARY_PORT_PLAN.md §7.3`, the API surface is stable:
//! callers pass `OfficeInput { cursus: Cursus::Cisterciensis, … }`
//! and the breviary entry points dispatch here. The Roman path is
//! the only one with a working body during the first parity pass —
//! Cistercian calls land in `unimplemented!()` so the unintended
//! fallback is loud rather than silent.
//!
//! When Cistercian support is added (post-B20, separate slice plan),
//! this file holds:
//!   - Cistercian capitulum overrides (Aestiva vs Hibernalis).
//!   - The `[Adv Ant 21L]` / `[Pasc P]` Cistercian-only antiphon keys.
//!   - The summer-ferial Ps 94 → Invitatorium swap referenced from
//!     `specials.pl:165-178`.

use crate::core::OfficeOutput;

/// Predicate consulted by the breviary specials walker to short-
/// circuit Cistercian-specific branches. First parity pass returns
/// `false` unconditionally — every Cistercian-only code path is
/// behind this guard, so flipping this to inspect
/// `office.cursus == Cursus::Cisterciensis` is the single point of
/// activation when the Cistercian port lands.
pub fn is_cistercian(_office: &OfficeOutput) -> bool {
    // Roman parity pass: never Cistercian. The `cursus` field on
    // `OfficeInput` exists per §7.2/§7.3 but every Cistercian-only
    // body lands in `unimplemented!()`.
    false
}
