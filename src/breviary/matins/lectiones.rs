//! Matins lectiones (1-9 readings) + Te Deum directive.
//!
//! Mirror of `specmatins.pl`:
//! - `lectiones($lang)` (line 651) — top-level loop over the 9 (or 3)
//!   lectiones, pairing each with its Absolutio + Benedictio +
//!   Responsory.
//! - `matins_lectio_responsory_alleluia($capit, $lang)` (line 693) —
//!   Easter-Alleluia injection at end of each responsory.
//! - `getC10readingname($lang)` (line 707) — special handling for
//!   Commune/C10 (Holy Cross) reading-name.
//! - `lectio` (ScriptFunc, line 718) — per-lectio body resolver.
//!   This is the densest single sub in the file (650+ LOC).
//! - `lectiones_ex3_fiunt4` (line 1370) — ferial scriptural-rotation
//!   helper for Sundays whose 1st nocturn lectiones get split.
//! - `tedeum_required($lang)` (line 1398) — predicate; returns true
//!   when the 9th responsory should be replaced by Te Deum.
//! - `contract_scripture` (line 1790) — abbreviated-scripture form.
//!
//! Today the basic 1-9 splice is in
//! `crate::horas::splice_matins_lectios`. The full port replaces it
//! with rubric-aware lectio + responsory pairing, scripture-rotation,
//! and absolutio/benedictio pre-roll.

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Top-level lectio loop. Emits the 1-9 readings with their pre-roll
/// (Pater + Absolutio + Benedictio) and intervening responsories.
///
/// Mirror of `lectiones($lang)` lines 651-692.
pub fn render_lectiones(_office: &OfficeOutput) -> Vec<RenderedLine> {
    // TODO(B19): port specmatins.pl:651-692.
    // Per-lectio composition:
    //   1. `Pater noster` macro
    //   2. Absolutio (per-rank, per-nocturn — one of three forms)
    //   3. Benedictio (per-position; benedictio 1, 2, 3, …, 9)
    //   4. Lectio body (via `lectio()` ScriptFunc port)
    //   5. Responsory (replaced by `Te Deum` for the 9th when
    //      `tedeum_required` returns true)
    unimplemented!("phase B19: render_lectiones")
}

/// Predicate: should the 9th responsory be replaced by Te Deum?
///
/// Mirror of `tedeum_required` lines 1398-1432.
///
/// Te Deum is suppressed when:
///   - Penitential season (Quad / Quadp / Adv outside vigils).
///   - Active feast carries `no Te Deum` in its `[Rule]`.
///   - Ferial day in pre-1955 use (with explicit exceptions for
///     Easter Octave, Pentecost Octave, etc.).
pub fn tedeum_required(_office: &OfficeOutput) -> bool {
    // TODO(B19): port specmatins.pl:1398-1432.
    unimplemented!("phase B19: tedeum_required")
}

/// Render absolutio + benedictio pair before a lectio. Per-nocturn.
///
/// Mirror of `get_absolutio_et_benedictiones` (line 484-650) — each
/// position has a different bene­dictio, with rubric variants.
pub fn get_absolutio_et_benedictiones(
    _office: &OfficeOutput,
    _lectio_idx: u8, // 1..=9
) -> Vec<RenderedLine> {
    // TODO(B19): port specmatins.pl:484-650.
    unimplemented!("phase B19: absolutio + benedictio")
}

/// Apply Easter-Alleluia injection to a responsory body. Mirror of
/// `matins_lectio_responsory_alleluia` line 693.
pub fn lectio_responsory_alleluia(_capit: &mut Vec<RenderedLine>, _lang: &str) {
    // TODO(B19): port specmatins.pl:693-706.
    unimplemented!("phase B19: matins_lectio_responsory_alleluia")
}

/// Abbreviate scripture body. Some `[Rule]` directives signal that
/// the day's 1st-nocturn lectiones should be the abbreviated form.
///
/// Mirror of `contract_scripture` line 1790.
pub fn contract_scripture(_body: &str) -> String {
    // TODO(B19): port specmatins.pl:1790-1812.
    unimplemented!("phase B19: contract_scripture")
}
