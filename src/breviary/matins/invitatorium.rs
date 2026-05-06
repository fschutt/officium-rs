//! Matins Invitatorium — Ps 94 + seasonal antiphon.
//!
//! Mirror of `specmatins.pl::invitatorium($lang)` (lines 26-151).
//!
//! Invitatorium structure:
//!   1. Antiphon — seasonal or per-feast (`Adoremus Dominum, qui fecit nos`
//!      default; `Christus natus est nobis` Christmas; `Surrexit Dominus
//!      vere` Easter; `Regem Apostolorum Dominum` Apostles' Common; etc.).
//!   2. Ps 94 (`Venite, exsultemus Domino`) with antiphon repeats
//!      between every other verse — the antiphon is interleaved into
//!      the psalm verses, not just framing it.
//!   3. Special "Christus surrexit" form during Easter Octave.
//!
//! The Cistercian rubric replaces Ps 94 with `Venite Exsultemus`
//! between Trinity Sunday and All Saints on ferial days within
//! Octaves; that branch is currently in `specials.pl:165-178` and
//! short-circuits to a plain Ps 94 macro.

use crate::core::OfficeOutput;
use crate::ordo::RenderedLine;

/// Render the Matins Invitatorium block.
///
/// Mirror of `specmatins.pl::invitatorium($lang)` (lines 26-151).
pub fn render_invitatorium(_office: &OfficeOutput) -> Vec<RenderedLine> {
    // TODO(B19): port specmatins.pl:26-151.
    // Composition:
    //   1. Resolve antiphon (via [`select_antiphon`]).
    //   2. Pull Ps 94 body from `crate::breviary::corpus::psalm("Psalm94")`.
    //   3. Interleave antiphon between verse pairs (the upstream Perl
    //      uses fixed verse numbers — verses 1-2, 3-5, 6-7, 8-9,
    //      10-11 — with antiphon repeats at fixed boundaries).
    //   4. Apply Easter Octave "Christus surrexit" override.
    unimplemented!("phase B19: render_invitatorium")
}

/// Pick the Invitatorium antiphon. Walks:
///   1. `Invit` from the day file (winner).
///   2. Commune fallback (`Invit` in `Commune/Cxx`).
///   3. `gettempora('Invit')` — season-keyed default.
///   4. The hard default `Adoremus Dominum, qui fecit nos`.
pub fn select_antiphon(_office: &OfficeOutput) -> String {
    // TODO(B19): port the antiphon-lookup chain.
    unimplemented!("phase B19: select_antiphon")
}
