//! Tempora keyword resolution — `gettempora`.
//!
//! Mirror of `vendor/divinum-officium/web/cgi-bin/horas/horascommon.pl::gettempora`
//! (lines 2243+). Used pervasively across the breviary helpers to
//! pick a season-keyed antiphon / capitulum / hymn override:
//!
//! - `gettempora('Capitulum Vespera')` returns the season-specific
//!   capitulum body (`[Adv]`, `[Quad]`, `[Pasc]` keys inside
//!   `Psalterium/Special/Major Special.txt`).
//! - `gettempora('Ant Matutinum')` does the same for nocturn antiphons.
//! - `gettempora('Hymnus Vespera')` for hymn overrides per season.
//!
//! The keying convention is a string suffix that each tempora-aware
//! file uses for its season-specific blocks.

use crate::core::OfficeOutput;

/// Resolve a tempora-keyed override for a section.
///
/// Returns the body of the season-specific block when the active
/// season has one, or `None` when no override exists (caller falls
/// back to the default).
///
/// Mirrors `gettempora($section)` lines 2243+. The exact Perl logic
/// reads `$dayname[0]` (the season tag, e.g. "Adv1", "Quad3", "Pasc0")
/// and walks a section-suffix match table:
///
/// | dayname[0] | suffix |
/// |---|---|
/// | `Adv1`–`Adv4` | ` Adv` |
/// | `Nat0`–`Nat6` | ` Nat` |
/// | `Epi1`–`Epi6` | ` Epi` |
/// | `Quadp1`–`Quadp3` | ` Quadp` |
/// | `Quad1`–`Quad6` | ` Quad` |
/// | `Pasc0`–`Pasc7` | ` Pasc` |
/// | `Pent01`–`Pent27` | ` Pent` |
/// | (default) | `` (empty — bare key) |
pub fn get_tempora(_office: &OfficeOutput, _section: &str) -> Option<String> {
    // TODO(B14): port horascommon.pl:2243+ (~100 LOC).
    // The implementation is a straightforward season-suffix lookup
    // against the per-day commune chain.
    unimplemented!("phase B14: gettempora")
}

/// The season suffix for the active office. Encapsulates the
/// `dayname[0]` → ` Adv`/` Quad`/` Pasc`/`...` mapping so other
/// helpers (Hymnus, Capitulum, Oratio) can build keyed lookups
/// without re-implementing the suffix table.
pub fn season_suffix(_output: &OfficeOutput) -> &'static str {
    // TODO(B14): port the season-suffix table.
    unimplemented!("phase B14: season_suffix")
}
