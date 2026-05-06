//! Matins — by far the largest single hour.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/specmatins.pl`
//! (1857 LOC of Perl). Slice B19 in the breviary plan.
//!
//! Matins composition (Tridentine / Divino):
//!
//! 1. **Invitatorium** — Ps 94 with seasonal antiphon.
//! 2. **Hymn** — `Aeterne rerum conditor` (default), seasonal
//!    overrides, festal overrides.
//! 3. **Three nocturns**, each:
//!    - 3 antiphons + 3 psalms (12 psalms total at festal Matins
//!      under Tridentine, 9 under Divino, varies under Monastic)
//!    - Versicle/response.
//!    - 3 lectiones, each preceded by `Pater noster` + Absolutio +
//!      Benedictio, followed by a responsory.
//! 4. **Te Deum** (suppressed on penitential days).
//!
//! Festal Matins has 9 lectiones; ferial Matins has 3 (one nocturn
//! only); 1-class feasts always have 9.
//!
//! Sub-modules:
//!   * [`invitatorium`] — Ps 94 + seasonal antiphon.
//!   * [`hymnus`] — Matins-specific hymn selection (separate from
//!     the general [`crate::breviary::hymnus`] because of the
//!     hymn-shift / hymn-merge logic).
//!   * [`nocturn`] — single-nocturn renderer.
//!   * [`psalmody`] — psalm selection + antiphon set per nocturn.
//!   * [`lectiones`] — 1-9 readings with rotation, scripture-of-the-day
//!     fallback, Te Deum directive.
//!   * [`responsory`] — short responsory + the 9th-responsory Te Deum
//!     swap.
//!   * [`initiarule`] — scripture-rotation table; when a feast displaces
//!     a Sunday, this decides what shifts where.

pub mod invitatorium;
pub mod hymnus;
pub mod nocturn;
pub mod psalmody;
pub mod lectiones;
pub mod responsory;
pub mod initiarule;

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Top-level Matins renderer. Composes the eight sub-pieces above.
///
/// Mirror of the Matins-specific path through `specials.pl::specials`
/// + `specmatins.pl`.
pub fn render_matins(_office: &OfficeOutput) -> Vec<RenderedLine> {
    // TODO(B19): orchestrate invitatorium → hymn → 3 nocturns →
    // Te Deum. Each nocturn is itself a composition; this fn is the
    // hour-level dispatcher.
    unimplemented!("phase B19: render_matins")
}
