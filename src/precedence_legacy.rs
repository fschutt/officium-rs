//! **LEGACY — see `precedence.rs` (Phase 4) for the canonical port.**
//!
//! Pragmatic 4-class approximation of the 1962 rubrics. Lives on as
//! the wired-up backend for `/wip/calendar` and `/wip/missal` so the
//! WIP pages keep rendering until **Phase 11** flips them to consume
//! `precedence::compute_office()`. Phase 11 deletes this file.
//!
//! TODO Phase 11: delete this file; switch `calendar.rs` / `missal.rs`
//! to `precedence::compute_office`.
//!
//! Original docstring follows.
//!
//! ---
//!
//! Concurrence / precedence rules — given today's temporal cycle slot
//! and (optionally) a sanctoral feast resolved by `kalendaria.rs`,
//! decide which of the two is the *primary* observance and which (if
//! any) is commemorated.
//!
//! This is a *pragmatic* simplification of the 1962 rubrics, calibrated
//! to produce correct output on the major fault lines (Easter Sunday,
//! Pentecost, Christmas, Sundays of Advent/Lent, ordinary Sundays,
//! ordinary ferias) while avoiding the full octave/concurrence machinery
//! the upstream Perl encodes. Specifically:
//!
//!   * **Class I temporal** (Easter Sunday, Pentecost Sunday, Christmas
//!     Day, Epiphany, etc.) always wins. Sanctoral commemorated only if
//!     the kalendaria says so.
//!   * **Class II temporal** (Sundays of Advent, Lent, Passion-tide,
//!     octave of Christmas, Pentecost-octave week) wins over sanctoral
//!     feasts of Class III and below; sanctoral commemorated.
//!   * **Class III temporal** (post-Pentecost Sundays, post-Epiphany
//!     Sundays) wins over sanctoral commemorations / simple feasts but
//!     yields to Class III sanctoral and above.
//!   * **Class IV temporal** (ordinary ferias) yields to any sanctoral
//!     entry.
//!
//! Out of scope here:
//!
//!   * First-vespers concurrence (today's vespers anticipating tomorrow's
//!     office). The page lookups are by-day, so first-vespers is a UI
//!     decision we'll add when the diurnal page lands.
//!   * Octave-day rules and "infra octavam" precedence — needs the
//!     octave bookkeeping the upstream code does in `directorium`.
//!   * The full table of Class I and II privileged ferias.

use crate::divinum_officium::kalendaria::Resolution;

/// Coarse precedence class. Lower = higher rank (Class I beats II).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    First = 1,
    Second = 2,
    Third = 3,
    Fourth = 4,
}

impl Class {
    pub fn label(self) -> &'static str {
        match self {
            Class::First => "I classis",
            Class::Second => "II classis",
            Class::Third => "III classis",
            Class::Fourth => "IV classis",
        }
    }
}

/// Map a temporal `getweek` code + weekday-of-week (0 = Sunday) to its
/// rubrical class. The codes come from `divinum_officium::date::getweek`.
pub fn temporal_class(week_code: &str, dow: u32, month: u32, day: u32) -> Class {
    let is_sunday = dow == 0;

    // Easter Sunday + the days of Easter Octave are Class I.
    if week_code == "Pasc0" {
        return Class::First;
    }
    // Pentecost Sunday is Class I.
    if week_code == "Pent00" {
        return Class::First;
    }
    // Pentecost octave's days (whole following week) are Class II in 1960.
    if week_code == "Pent01" && !is_sunday {
        // The week of Pent01 is the "Hebdomada infra octavam Pentecostes"
        // until Trinity Sunday — class II privileged.
        return Class::Second;
    }
    // Christmas, Epiphany, etc. — handled via the sanctoral side
    // (their Sancti rank_num is high enough to claim Class I on its own).
    let _ = (month, day);

    if week_code.starts_with("Pasc") {
        // Eastertide outside the octave. Sundays = Class II, ferias = IV.
        return if is_sunday { Class::Second } else { Class::Fourth };
    }
    if week_code.starts_with("Adv") {
        // Sundays of Advent = Class II. Ferias of 17–24 Dec are Class II
        // privileged in 1962, but we don't try to pick those out here.
        return if is_sunday { Class::Second } else { Class::Fourth };
    }
    if week_code.starts_with("Quad") {
        // Sundays of Lent = Class I (Palm Sunday) / II (others). Ferias
        // of Lent are Class III privileged (no sanctoral overrides).
        // Quad6 = Holy Week; treat as Class I-equivalent.
        if week_code == "Quad6" {
            return Class::First;
        }
        return if is_sunday { Class::Second } else { Class::Third };
    }
    if week_code.starts_with("Quadp") {
        // Septuagesima / Sexagesima / Quinquagesima Sundays = Class II;
        // ferias of those weeks = Class IV.
        return if is_sunday { Class::Second } else { Class::Fourth };
    }
    if week_code.starts_with("Pent") {
        // Post-Pentecost Sundays = Class III, ferias = Class IV.
        return if is_sunday { Class::Third } else { Class::Fourth };
    }
    if week_code.starts_with("Epi") {
        return if is_sunday { Class::Third } else { Class::Fourth };
    }
    if week_code.starts_with("Nat") {
        // Christmas Octave days: Class II privileged, but the high-rank
        // sanctoral entries (Octave Day, Holy Innocents, etc.) usually
        // claim Class I/II from the sanctoral side. Default Class III.
        return Class::Third;
    }
    Class::Fourth
}

/// Map a sanctoral `rank_num` to its precedence class. The numbering
/// convention in the upstream Sancti corpus:
///
///   `≥6` → Class I (Christmas, Easter, Pentecost, major patrons w/octave)
///   `≥5` → Class II (e.g. Joseph the Worker, Apostles in some uses)
///   `≥2` → Class III (Duplex / Semiduplex)
///   `<2` → Class IV (Simple, Commemoration)
pub fn sanctoral_class(rank_num: Option<f32>) -> Class {
    let r = rank_num.unwrap_or(0.0);
    if r >= 6.0 {
        Class::First
    } else if r >= 5.0 {
        Class::Second
    } else if r >= 2.0 {
        Class::Third
    } else {
        Class::Fourth
    }
}

/// Which side wins today.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Winner {
    /// Temporal cycle (Sunday, season day, Easter, etc.) is primary.
    Temporal,
    /// Sanctoral feast is primary.
    Sanctoral,
    /// No contest — either temporal-only (no Sancti entry) or
    /// sanctoral-only (rare; here only when temporal_class is IV).
    NoContest,
}

/// Decide the winner. Tie-breaker rules:
///
///   * If sanctoral resolution is `Suppressed` or `Ferial`, temporal
///     wins by default (no contest with the sanctoral side).
///   * If they have equal class, sanctoral wins (the page reflects the
///     fact that we bothered to render a Sancti entry).
///   * Otherwise the lower-numbered class wins.
pub fn decide(temporal: Class, sanctoral: &Resolution) -> Winner {
    let s_rank = match sanctoral {
        Resolution::Suppressed | Resolution::Ferial => return Winner::Temporal,
        Resolution::Override(e) => e.main.rank_num,
        Resolution::Default(e) => e.rank_num,
    };
    let s_class = sanctoral_class(s_rank);
    if temporal < s_class {
        Winner::Temporal
    } else {
        // equal-or-lower temporal class → sanctoral wins
        Winner::Sanctoral
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::divinum_officium::kalendaria::resolve_1962;

    #[test]
    fn easter_sunday_wins() {
        // Pasc0 + sanctoral 04-05 (S. Vincentii Ferrerii Confessoris,
        // Duplex)
        let temp = temporal_class("Pasc0", 0, 4, 5);
        assert_eq!(temp, Class::First);
        let sanct = resolve_1962(4, 5);
        assert_eq!(decide(temp, &sanct), Winner::Temporal);
    }

    #[test]
    fn ordinary_sunday_yields_to_class_iii_saint() {
        // 03 May 2026: Hebdomada IV post Pascha (Pasc4) + Ss. Alexandri
        // martyrs (rank 1, Class IV). Sunday Class II vs sanctoral
        // Class IV → Sunday wins.
        let temp = temporal_class("Pasc4", 0, 5, 3);
        let sanct = resolve_1962(5, 3);
        assert_eq!(decide(temp, &sanct), Winner::Temporal);
    }

    #[test]
    fn weekday_yields_to_class_iii_saint() {
        // Wed 29 Apr 2026: Pasc3 + S. Petri Martyris (Duplex, rank 3,
        // Class III). Weekday is IV → saint wins.
        let temp = temporal_class("Pasc3", 3, 4, 29);
        assert_eq!(temp, Class::Fourth);
        let sanct = resolve_1962(4, 29);
        assert_eq!(decide(temp, &sanct), Winner::Sanctoral);
    }

    #[test]
    fn ferial_with_no_sanctoral() {
        // 6 May 2026: feria, no sanctoral (1962 reform suppressed).
        let temp = temporal_class("Pasc4", 3, 5, 6);
        let sanct = resolve_1962(5, 6);
        assert_eq!(decide(temp, &sanct), Winner::Temporal);
    }
}
